//! Standalone launcher's filesystem operations, independent of the file picker.
use std::{ffi::c_void, fs, io::{Read, Seek, SeekFrom, Write}, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
use std::os::windows::fs::MetadataExt;

pub type Result<T> = std::result::Result<T, String>;
pub struct Payload<'a> { pub dll: &'a [u8], pub files: &'a [(&'a str, &'a [u8])] }
pub const DATA_DIRECTORY: &str = "MaJmods";
pub const LEGACY_DATA_DIRECTORY: &str = "MJLocalPreview";
pub const DLL_FILE_NAME: &str = "MaJmods.dll";

fn io_error(e: impl std::fmt::Display) -> String { format!("{e}") }
fn wide(path: &Path) -> Vec<u16> { use std::os::windows::ffi::OsStrExt; path.as_os_str().encode_wide().chain(Some(0)).collect() }
fn unique() -> String { format!("{}-{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos(), std::process::id()) }
pub fn unlinked(path: &Path) -> Result<()> {
    if path.exists() && fs::symlink_metadata(path).map_err(io_error)?.file_attributes() & 0x400 != 0 {
        return Err(format!("不支持链接目录或文件：{}", path.display()));
    }
    Ok(())
}
pub fn data_directory(root: &Path) -> Result<PathBuf> {
    let data = root.join(DATA_DIRECTORY);
    unlinked(&data)?;
    if data.is_dir() { return Ok(data); }
    if data.exists() { return Err(format!("无法使用 {} 目录，已有同名文件", data.display())); }
    let legacy = root.join(LEGACY_DATA_DIRECTORY);
    if legacy.is_dir() {
        unlinked(&legacy)?;
        if let Err(error) = fs::rename(&legacy, &data) {
            unlinked(&data)?;
            if legacy.exists() || !data.is_dir() {
                return Err(format!("无法将 {} 改名为 {}：{error}", legacy.display(), data.display()));
            }
        }
    }
    Ok(data)
}
pub fn normal_path(path: &Path) -> Result<PathBuf> {
    let p = fs::canonicalize(path).map_err(io_error)?;
    let s = p.to_str().ok_or("路径包含无法保存的字符")?;
    Ok(if let Some(unc) = s.strip_prefix(r"\\?\UNC\") { PathBuf::from(format!(r"\\{unc}")) }
       else { PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(s)) })
}
pub fn pe_machine(path: &Path) -> Result<u16> {
    let mut file = fs::File::open(path).map_err(io_error)?;
    let mut dos = [0u8; 64]; file.read_exact(&mut dos).map_err(io_error)?;
    if &dos[..2] != b"MZ" { return Err("请选择有效的游戏 EXE 文件".into()); }
    let offset = u32::from_le_bytes(dos[60..64].try_into().unwrap());
    if !(64..=1_048_576).contains(&offset) { return Err("游戏 EXE 格式不正确".into()); }
    file.seek(SeekFrom::Start(offset as u64)).map_err(io_error)?;
    let mut pe = [0u8; 6]; file.read_exact(&mut pe).map_err(io_error)?;
    if &pe[..4] != b"PE\0\0" { return Err("游戏 EXE 格式不正确".into()); }
    Ok(u16::from_le_bytes([pe[4], pe[5]]))
}
pub fn validate_game(exe: &Path) -> Result<(PathBuf, PathBuf)> {
    if !exe.is_absolute() || !exe.is_file() || !exe.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
        return Err("游戏路径已失效，请重新选择雀魂启动程序（EXE）".into());
    }
    let exe = normal_path(exe)?;
    pe_machine(&exe)?;
    let parent = exe.parent().ok_or("无法读取游戏目录")?;
    let nested = parent.join("GameClient");
    let game = if parent.join("GameAssembly.dll").is_file() { parent.to_path_buf() } else { nested };
    for file in ["GameAssembly.dll", "UnityPlayer.dll", "Jantama_MahjongSoul_Data/Plugins/x86/tolua.dll"] {
        if !game.join(file).is_file() { return Err("这个位置不是支持的雀魂客户端，请选择 Steam 安装目录或官方版 GameClient 中的游戏 EXE".into()); }
    }
    let client = if game == parent { exe.clone() } else { game.join("Jantama_MahjongSoul.exe") };
    if pe_machine(&client)? != 0x14c { return Err("当前试用工具支持 32 位雀魂客户端".into()); }
    unlinked(&game)?;
    Ok((exe, game))
}
pub fn load_path(settings: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(settings).ok()?;
    if text.len() > 131072 { return None; }
    let text=text.trim_start_matches('\u{feff}').trim_end_matches(['\r', '\n']);
    let p = PathBuf::from(text.strip_prefix("game=").unwrap_or(text));
    p.is_absolute().then_some(p)
}
pub fn save_path(settings: &Path, exe: &Path) -> Result<()> {
    let s = exe.to_str().ok_or("无法保存路径")?;
    if !exe.is_absolute() || s.contains(['\r', '\n', '\0']) { return Err("游戏路径无效".into()); }
    fs::create_dir_all(settings.parent().ok_or("无法创建设置目录")?).map_err(io_error)?;
    unlinked(settings)?;
    // Path preferences are recoverable: a missing/corrupt record simply reprompts.
    // Write directly so encrypted/redirected user-profile folders need no rename.
    let mut file=fs::File::create(settings).map_err(io_error)?;
    let text=if settings.extension().is_some_and(|e|e.eq_ignore_ascii_case("ini")){format!("game={s}\r\n")}else{s.to_string()};
    file.write_all(text.as_bytes()).and_then(|_|file.sync_all()).map_err(io_error)
}

#[link(name="kernel32")]
extern "system" { fn MoveFileExW(from:*const u16, to:*const u16, flags:u32) -> i32; }
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    unlinked(path)?;
    let name = path.file_name().ok_or("无效文件名")?.to_string_lossy();
    let temp = path.with_file_name(format!("{name}.{}.tmp", unique()));
    let mut file = fs::OpenOptions::new().create_new(true).write(true).open(&temp).map_err(io_error)?;
    file.write_all(bytes).and_then(|_| file.sync_all()).map_err(io_error)?; drop(file);
    let from=wide(&temp); let to=wide(path);
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 1|8) } == 0 {
        let err = std::io::Error::last_os_error();
        let _ = fs::remove_file(&temp);
        return Err(format!("无法写入 {}：{err}", path.display()));
    }
    Ok(())
}

#[link(name="bcrypt")]
extern "system" {
    fn BCryptOpenAlgorithmProvider(out:*mut *mut c_void, id:*const u16, implementation:*const u16, flags:u32)->i32;
    fn BCryptCloseAlgorithmProvider(handle:*mut c_void, flags:u32)->i32;
    fn BCryptGetProperty(handle:*mut c_void, name:*const u16, out:*mut u8, len:u32, used:*mut u32, flags:u32)->i32;
    fn BCryptCreateHash(alg:*mut c_void, hash:*mut *mut c_void, object:*mut u8, len:u32, secret:*const u8, secret_len:u32, flags:u32)->i32;
    fn BCryptHashData(hash:*mut c_void, input:*const u8, len:u32, flags:u32)->i32;
    fn BCryptFinishHash(hash:*mut c_void, out:*mut u8, len:u32, flags:u32)->i32;
    fn BCryptDestroyHash(hash:*mut c_void)->i32;
}
pub fn sha256(bytes:&[u8]) -> Result<String> { unsafe {
    struct Alg(*mut c_void); impl Drop for Alg { fn drop(&mut self) { unsafe { BCryptCloseAlgorithmProvider(self.0,0); } } }
    struct Hash(*mut c_void); impl Drop for Hash { fn drop(&mut self) { unsafe { BCryptDestroyHash(self.0); } } }
    let id:Vec<u16> = "SHA256\0".encode_utf16().collect();
    let mut alg = std::ptr::null_mut();
    if BCryptOpenAlgorithmProvider(&mut alg,id.as_ptr(),std::ptr::null(),0)<0 { return Err("无法计算安装校验值".into()); }
    let alg = Alg(alg); let mut len=0u32; let mut used=0;
    let prop:Vec<u16>="ObjectLength\0".encode_utf16().collect();
    if BCryptGetProperty(alg.0,prop.as_ptr(),(&mut len as *mut u32).cast(),4,&mut used,0)<0 || len>1_048_576 { return Err("无法读取校验属性".into()); }
    let mut object=vec![0u8;len as usize]; let mut hash=std::ptr::null_mut();
    if BCryptCreateHash(alg.0,&mut hash,object.as_mut_ptr(),len,std::ptr::null(),0,0)<0 { return Err("无法创建安装校验".into()); }
    let hash=Hash(hash); let mut result=[0u8;32];
    for chunk in bytes.chunks(1_048_576) {
        if BCryptHashData(hash.0,chunk.as_ptr(),chunk.len() as u32,0)<0 { return Err("安装校验失败".into()); }
    }
    if BCryptFinishHash(hash.0,result.as_mut_ptr(),32,0)<0 { return Err("安装校验失败".into()); }
    Ok(result.iter().map(|v|format!("{v:02X}")).collect())
} }

fn json_string(value:&str)->String {
    format!("\"{}\"", value.replace('\\',"\\\\").replace('"',"\\\"").replace('\r',"\\r").replace('\n',"\\n"))
}
fn updated_manifest(text:&str, hash:&str)->Option<String> {
    // Preserve the existing PowerShell installer's original restore chain.
    let start=text.find("\"hash\"")?+6;
    let colon=text[start..].find(':')?+start+1;
    let quote=text[colon..].find('"')?+colon+1;
    let end=text[quote..].find('"')?+quote;
    if end-quote!=64 || !text[quote..end].bytes().all(|c|c.is_ascii_hexdigit()) { return None; }
    Some(format!("{}{}{}",&text[..quote],hash,&text[end..]))
}
fn copy_tree(from:&Path, to:&Path)->Result<()> {
    unlinked(from)?; fs::create_dir(to).map_err(io_error)?;
    for item in fs::read_dir(from).map_err(io_error)? {
        let item=item.map_err(io_error)?; unlinked(&item.path())?;
        let target=to.join(item.file_name());
        if item.file_type().map_err(io_error)?.is_dir() { copy_tree(&item.path(),&target)?; }
        else { fs::copy(item.path(),target).map_err(io_error)?; }
    }
    Ok(())
}

fn manifest_string(text:&str,key:&str)->Option<String> {
    let start=text.find(&format!("\"{key}\""))?+key.len()+2;
    let tail=text[start..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let mut out=String::new();let mut chars=tail.chars();
    while let Some(c)=chars.next() {
        match c {
            '"'=>return Some(out),
            '\\'=>out.push(match chars.next()? {'\\'=>'\\','"'=>'"','/'=>'/','n'=>'\n','r'=>'\r','t'=>'\t',_=>return None}),
            c if c.is_control()=>return None,
            c=>out.push(c),
        }
    }
    None
}
fn contains(bytes:&[u8], value:&[u8])->bool {bytes.windows(value.len()).any(|p|p==value)}

fn installation_directory(game: &Path) -> PathBuf {
    let legacy = game.join(LEGACY_DATA_DIRECTORY);
    let data = game.join(DATA_DIRECTORY);
    if !legacy.join("install.json").exists() && data.join("install.json").exists() { data } else { legacy }
}

/// Migrate only a hash-verified installation of our earlier version.dll loader.
/// Preserve its data, original restore chain and removed DLL; never remove an unknown DLL.
pub fn retire_legacy(game:&Path)->Result<Option<PathBuf>> {
    unlinked(game)?;
    let dll=game.join("version.dll");
    if !dll.exists() { return Ok(None); }
    unlinked(&dll)?;
    let bytes=fs::read(&dll).map_err(io_error)?;
    if !contains(&bytes,b"MJLocalPreview") { return Ok(None); }
    let data=installation_directory(game);let manifest=data.join("install.json");
    unlinked(&data)?;unlinked(&manifest)?;
    let text=fs::read_to_string(&manifest).map_err(|_|"旧版试用 DLL 缺少安装记录，无法安全切换为注入版".to_string())?;
    if text.len()>16384 || manifest_string(&text,"hash").is_none_or(|hash|!hash.eq_ignore_ascii_case(&sha256(&bytes).unwrap_or_default())) {
        return Err("旧版试用 DLL 与安装记录不符，已保留原文件，请先恢复旧版安装".into());
    }
    let restore=game.join("MJLocalPreview.restore");unlinked(&restore)?;
    let backup=PathBuf::from(manifest_string(&text,"backup").ok_or("旧版备份记录无效")?);
    unlinked(&backup)?;
    let resolved=normal_path(&backup)?;let restore_root=normal_path(&restore)?;
    if !resolved.starts_with(&restore_root) || resolved==restore_root {return Err("旧版备份不在指定恢复目录中".into());}
    let original=resolved.join("previous-version.dll");unlinked(&original)?;
    let original_bytes=if original.is_file() {Some(fs::read(&original).map_err(io_error)?)} else {None};
    // The discarded prototype is also a cosmetic injector, not a system dependency.
    let restore_original=original_bytes.as_ref().is_some_and(|b|!contains(b,b"MJLocalPreview")&&!contains(b,b"MajsoulPreview"));
    let archive=restore.join(format!("injected-migration-{}",unique()));fs::create_dir(&archive).map_err(io_error)?;
    let retired=archive.join("version.dll");
    fs::rename(&dll,&retired).map_err(|e|format!("请先退出游戏，旧版试用 DLL 无法移出：{e}"))?;
    if restore_original {
        if let Err(e)=atomic_write(&dll,original_bytes.as_ref().unwrap()) {
            fs::rename(&retired,&dll).map_err(|rollback|format!("{e}；旧 DLL 恢复失败：{rollback}；备份 {}",retired.display()))?;
            return Err(e);
        }
    }
    Ok(Some(retired))
}

pub fn deploy(game:&Path, payload:&Payload<'_>)->Result<bool> {
    let data=installation_directory(game); let restore=game.join("MJLocalPreview.restore");
    for p in [game,&data,&restore] { unlinked(p)?; }
    let dll=game.join("version.dll"); let manifest=data.join("install.json"); let marker=data.join("launcher-payload.sha256");
    let mut bundled=payload.dll.to_vec();
    for (name,bytes) in payload.files {
        if Path::new(name).components().count()!=1 || !["runtime.lua","adapter.tsv","config.ini","max_data.yaml"].contains(name) { return Err("程序内的安装数据无效".into()); }
        bundled.extend_from_slice(name.as_bytes()); bundled.extend_from_slice(bytes);
        unlinked(&data.join(name))?;
    }
    unlinked(&dll)?; unlinked(&manifest)?; unlinked(&marker)?;
    let fingerprint=sha256(&bundled)?;
    if fs::read_to_string(&marker).ok().as_deref()==Some(&fingerprint) &&
       fs::read(&dll).ok().as_deref()==Some(payload.dll) && payload.files.iter().all(|(name,_)|data.join(name).is_file()) {
        // Runtime/adapter/catalog/config are external data. Respect manual updates
        // on subsequent runs of this same bundled version.
        return Ok(false);
    }
    let hash=sha256(payload.dll)?;
    let old_manifest=if manifest.exists() { Some(fs::read_to_string(&manifest).map_err(io_error)?) } else { None };
    let retained=old_manifest.as_deref().map(|text|updated_manifest(text,&hash).ok_or("安装记录格式不正确，已保留原文件，请先恢复原安装".to_string())).transpose()?;
    let had_dll=dll.exists(); let had_data=data.exists();
    fs::create_dir_all(&restore).map_err(io_error)?;
    let backup=restore.join(unique()); fs::create_dir(&backup).map_err(io_error)?;
    if had_dll { fs::copy(&dll,backup.join("previous-version.dll")).map_err(io_error)?; }
    if had_data { copy_tree(&data,&backup.join("previous-data"))?; }
    let generated=format!("{{\n  \"backup\": {},\n  \"hadDll\": {had_dll},\n  \"hadData\": {had_data},\n  \"hash\": {}\n}}\n",json_string(&backup.to_string_lossy()),json_string(&hash));
    let new_manifest=retained.unwrap_or(generated);
    let mut writes:Vec<(PathBuf,Vec<u8>)>=vec![(dll,payload.dll.to_vec())];
    for (name,bytes) in payload.files {
        let target=data.join(name);
        if (*name=="config.ini" || *name=="max_data.yaml") && target.exists() { continue; }
        writes.push((target,bytes.to_vec()));
    }
    writes.push((manifest,new_manifest.into_bytes())); writes.push((marker,fingerprint.into_bytes()));
    // Read all preimages before changing any file. Commit DLL first so a running
    // client's file lock causes no partial data upgrade.
    let previous:Vec<_>=writes.iter().map(|(p,_)|if p.exists(){fs::read(p).map(Some).map_err(io_error)}else{Ok(None)}).collect::<Result<_>>()?;
    fs::create_dir_all(&data).map_err(io_error)?;
    for (index,(path,bytes)) in writes.iter().enumerate() {
        if let Err(error)=atomic_write(path,bytes) {
            let mut failures=Vec::new();
            for j in (0..index).rev() {
                let target=&writes[j].0;
                let reverted=if let Some(old)=&previous[j] { atomic_write(target,old) }
                    else { fs::rename(target,backup.join(format!("partial-{j}"))).map_err(io_error) };
                if let Err(e)=reverted { failures.push(e); }
            }
            return Err(format!("{error}\n旧文件已备份：{}{}",backup.display(),if failures.is_empty(){String::new()}else{format!("\n恢复失败：{}",failures.join("；"))}));
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn directory()->PathBuf {
        // Kept inside this project's target folder, including failure evidence.
        let p=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("launcher-check-{}",unique()));
        fs::create_dir_all(&p).unwrap(); p
    }
    fn exe(path:&Path,machine:u16) {
        let mut pe=vec![0u8;70];pe[..2].copy_from_slice(b"MZ");pe[60..64].copy_from_slice(&64u32.to_le_bytes());
        pe[64..68].copy_from_slice(b"PE\0\0");pe[68..70].copy_from_slice(&machine.to_le_bytes());fs::write(path,pe).unwrap();
    }
    fn payload(dll:&[u8])->Payload<'_> { Payload { dll,files:&[("runtime.lua",b"runtime"),("adapter.tsv",b"adapter"),("config.ini",b"config"),("max_data.yaml",b"catalog")] } }
    #[test] fn data_directory_migrates_existing_mods_once() {
        let root = directory();
        let data = root.join(DATA_DIRECTORY);
        let legacy = root.join(LEGACY_DATA_DIRECTORY);
        let config = "mods/中文角色/皮肤/mod.ini";
        let legacy_config = legacy.join(config);
        fs::create_dir_all(legacy_config.parent().unwrap()).unwrap();
        fs::write(&legacy_config, b"enabled = 0").unwrap();
        assert_eq!(data_directory(&root).unwrap(), data);
        assert!(!legacy.exists());
        assert_eq!(fs::read(data.join(config)).unwrap(), b"enabled = 0");
        fs::create_dir(&legacy).unwrap();
        fs::write(legacy.join("config.ini"), b"separate old settings").unwrap();
        assert_eq!(data_directory(&root).unwrap(), data);
        assert!(legacy.join("config.ini").is_file());
        assert!(!data.join("config.ini").exists());
    }
    #[test] fn data_directory_rejects_file_in_place_of_new_folder() {
        let root = directory();
        let legacy = root.join(LEGACY_DATA_DIRECTORY);
        fs::create_dir(&legacy).unwrap();
        fs::write(root.join(DATA_DIRECTORY), b"existing file").unwrap();
        assert!(data_directory(&root).is_err());
        assert!(legacy.is_dir());
        assert_eq!(fs::read(root.join(DATA_DIRECTORY)).unwrap(), b"existing file");
    }
    #[test] fn validates_unicode_path_and_remembers_it() {
        let root=directory();let game=root.join("雀魂 测试");fs::create_dir(&game).unwrap();
        let launch=game.join("Jantama_MahjongSoul.exe");exe(&launch,0x14c);
        for f in ["GameAssembly.dll","UnityPlayer.dll"] {fs::write(game.join(f),b"test").unwrap();}
        let plugins=game.join("Jantama_MahjongSoul_Data/Plugins/x86");fs::create_dir_all(&plugins).unwrap();fs::write(plugins.join("tolua.dll"),b"test").unwrap();
        let (normalized,dir)=validate_game(&launch).unwrap();assert_eq!(dir,game);
        let settings=root.join("settings/game-path.txt");save_path(&settings,&normalized).unwrap();assert_eq!(load_path(&settings),Some(normalized));
        exe(&launch,0x8664);assert!(validate_game(&launch).is_err());
        assert!(validate_game(&root.join("missing.exe")).is_err());
    }
    #[test] fn deployment_preserves_catalog_settings_and_manual_updates() {
        let game=directory();let first=payload(b"dll-v1");assert!(deploy(&game,&first).unwrap());
        let data=game.join("MJLocalPreview");let original=fs::read_to_string(data.join("install.json")).unwrap();
        fs::write(data.join("max_data.yaml"),b"user catalog").unwrap();fs::write(data.join("config.ini"),b"user settings").unwrap();fs::write(data.join("runtime.lua"),b"user adapter update").unwrap();
        assert!(!deploy(&game,&first).unwrap());assert_eq!(fs::read(data.join("runtime.lua")).unwrap(),b"user adapter update");
        assert!(deploy(&game,&payload(b"dll-v2")).unwrap());
        assert_eq!(fs::read(data.join("max_data.yaml")).unwrap(),b"user catalog");assert_eq!(fs::read(data.join("config.ini")).unwrap(),b"user settings");
        assert_eq!(fs::read_to_string(data.join("install.json")).unwrap(),updated_manifest(&original,&sha256(b"dll-v2").unwrap()).unwrap());
        assert_eq!(fs::read_dir(game.join("MJLocalPreview.restore")).unwrap().count(),2);
    }
    #[test] fn first_install_keeps_previous_files_and_compatible_manifest() {
        let game=directory();let data=game.join("MJLocalPreview");fs::create_dir(&data).unwrap();
        fs::write(game.join("version.dll"),b"previous dll").unwrap();fs::write(data.join("notes.txt"),b"previous extra file").unwrap();
        deploy(&game,&payload(b"new dll")).unwrap();
        let backup=fs::read_dir(game.join("MJLocalPreview.restore")).unwrap().next().unwrap().unwrap().path();
        assert_eq!(fs::read(backup.join("previous-version.dll")).unwrap(),b"previous dll");assert_eq!(fs::read(backup.join("previous-data/notes.txt")).unwrap(),b"previous extra file");
        let manifest=fs::read_to_string(data.join("install.json")).unwrap();assert!(manifest.contains("\"hadDll\": true")&&manifest.contains("\"hadData\": true"));
    }
    #[test] fn locked_dll_cannot_leave_a_partial_upgrade() {
        use std::os::windows::fs::OpenOptionsExt;
        let game=directory();deploy(&game,&payload(b"old dll")).unwrap();
        let data=game.join("MJLocalPreview");fs::write(data.join("runtime.lua"),b"current runtime").unwrap();
        let lock=fs::OpenOptions::new().read(true).share_mode(0).open(game.join("version.dll")).unwrap();
        assert!(deploy(&game,&payload(b"new dll")).is_err());drop(lock);
        assert_eq!(fs::read(game.join("version.dll")).unwrap(),b"old dll");assert_eq!(fs::read(data.join("runtime.lua")).unwrap(),b"current runtime");
    }
    #[test] fn windows_hash_matches_sha256_and_manifest_changes_only_hash() {
        assert_eq!(sha256(b"abc").unwrap(),"BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD");
        assert!(updated_manifest("{}","test").is_none());
    }
    #[test] fn migration_restores_original_and_rejects_modified_loader() {
        let game=directory();let data=game.join("MJLocalPreview");let backup=game.join("MJLocalPreview.restore/original");
        fs::create_dir_all(&data).unwrap();fs::create_dir_all(&backup).unwrap();
        let loader=b"MJLocalPreview old loader";fs::write(game.join("version.dll"),loader).unwrap();
        fs::write(backup.join("previous-version.dll"),b"original vendor dll").unwrap();
        let state=format!("{{\"backup\":{},\"hash\":{}}}",json_string(backup.to_str().unwrap()),json_string(&sha256(loader).unwrap()));
        fs::write(data.join("install.json"),&state).unwrap();fs::write(data.join("max_data.yaml"),b"custom catalogue").unwrap();
        fs::write(game.join("version.dll"),b"MJLocalPreview modified").unwrap();
        assert!(retire_legacy(&game).is_err());assert_eq!(fs::read(game.join("version.dll")).unwrap(),b"MJLocalPreview modified");
        fs::write(game.join("version.dll"),loader).unwrap();let saved=retire_legacy(&game).unwrap().unwrap();
        assert_eq!(fs::read(saved).unwrap(),loader);assert_eq!(fs::read(game.join("version.dll")).unwrap(),b"original vendor dll");
        assert_eq!(fs::read(data.join("max_data.yaml")).unwrap(),b"custom catalogue");assert!(retire_legacy(&game).unwrap().is_none());
    }
    #[test] fn migration_keeps_obsolete_prototype_in_backup_instead_of_reactivating_it() {
        let game=directory();let backup=game.join("MJLocalPreview.restore/original");let data=game.join(DATA_DIRECTORY);
        fs::create_dir_all(&backup).unwrap();fs::create_dir_all(&data).unwrap();let loader=b"MJLocalPreview previous";
        fs::write(game.join("version.dll"),loader).unwrap();fs::write(backup.join("previous-version.dll"),b"MajsoulPreview obsolete").unwrap();
        fs::write(data.join("install.json"),format!("{{\"backup\":{},\"hash\":{}}}",json_string(backup.to_str().unwrap()),json_string(&sha256(loader).unwrap()))).unwrap();
        assert!(retire_legacy(&game).unwrap().is_some());assert!(!game.join("version.dll").exists());assert!(backup.join("previous-version.dll").exists());
    }
}
