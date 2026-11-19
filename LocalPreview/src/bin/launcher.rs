#![allow(non_snake_case)]

use std::{ffi::{c_void, OsString}, path::{Path, PathBuf}, ptr, io::Write, time::{Duration,Instant}};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use version::{bundled,injection,launcher::{self,Result}};
fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(Some(0)).collect()}

// Classic OPENFILENAMEW (88 bytes on x86); accepted by modern Windows.
#[repr(C)]
struct OpenFileName {
    size:u32, owner:*mut c_void, instance:*mut c_void, filter:*const u16,
    custom_filter:*mut u16, max_custom_filter:u32, filter_index:u32, file:*mut u16,
    max_file:u32, file_title:*mut u16, max_file_title:u32, initial_dir:*const u16,
    title:*const u16, flags:u32, file_offset:u16, file_extension:u16,
    default_extension:*const u16, custom_data:isize, hook:*mut c_void,
    template_name:*const u16, reserved:*mut c_void, reserved_word:u32, flags_ex:u32,
}
#[link(name="comdlg32")]
extern "system" { fn GetOpenFileNameW(data:*mut OpenFileName)->i32; fn CommDlgExtendedError()->u32; }
#[link(name="user32")]
extern "system" { fn MessageBoxW(owner:*mut c_void,text:*const u16,title:*const u16,kind:u32)->i32; }
#[link(name="kernel32")]
extern "system" {
    fn CreateMutexW(attributes:*mut c_void,own:i32,name:*const u16)->*mut c_void;
    fn GetLastError()->u32;
    fn CloseHandle(handle:*mut c_void)->i32;
    fn CreateToolhelp32Snapshot(flags:u32,pid:u32)->*mut c_void;
    fn Process32FirstW(snapshot:*mut c_void,entry:*mut ProcessEntry)->i32;
    fn Process32NextW(snapshot:*mut c_void,entry:*mut ProcessEntry)->i32;
    fn OpenProcess(access:u32,inherit:i32,pid:u32)->*mut c_void;
    fn QueryFullProcessImageNameW(process:*mut c_void,flags:u32,buffer:*mut u16,size:*mut u32)->i32;
    fn SetConsoleOutputCP(code:u32)->i32;
    fn SetConsoleTitleW(title:*const u16)->i32;
    fn GetConsoleProcessList(list:*mut u32,size:u32)->u32;
}
#[repr(C)]
struct ProcessEntry {
    size:u32, usage:u32, pid:u32, heap:usize, module:u32, threads:u32,
    parent:u32, priority:i32, flags:u32, exe:[u16;260],
}
struct Handle(*mut c_void);
impl Drop for Handle {fn drop(&mut self){if !self.0.is_null() && self.0 as isize!=-1 {unsafe{CloseHandle(self.0);}}}}
fn message(text:&str) {unsafe {MessageBoxW(ptr::null_mut(),wide(text).as_ptr(),wide("雀魂本地装扮试用").as_ptr(),0x40);}}

fn choose(previous:Option<&Path>)->Result<Option<PathBuf>> {
    let title=wide("请选择雀魂游戏启动程序（首次选择后会记住路径）");
    let filter=wide("雀魂游戏程序 (*.exe)\0*.exe\0");
    let folder=previous.and_then(Path::parent).filter(|p|p.is_dir()).map(Path::to_path_buf)
        .or_else(||std::env::var_os("USERPROFILE").map(PathBuf::from));
    let initial:Option<Vec<u16>>=folder.map(|p|p.as_os_str().encode_wide().chain(Some(0)).collect());
    let mut file=vec![0u16;32768];
    let mut data:OpenFileName=unsafe{std::mem::zeroed()};
    data.size=std::mem::size_of::<OpenFileName>() as u32;data.filter=filter.as_ptr();data.filter_index=1;
    data.file=file.as_mut_ptr();data.max_file=file.len() as u32;
    data.initial_dir=initial.as_ref().map_or(ptr::null(),|s|s.as_ptr());data.title=title.as_ptr();
    data.flags=0x80000|0x1000|0x800|0x8|0x02000000; // Explorer, existing file/path, no cwd/recent changes.
    if unsafe{GetOpenFileNameW(&mut data)}==0 {
        let code=unsafe{CommDlgExtendedError()};
        return if code==0 {Ok(None)}else{Err(format!("无法打开文件选择窗口（错误 {code}）"))};
    }
    let len=file.iter().position(|c|*c==0).unwrap_or(file.len());
    Ok(Some(PathBuf::from(OsString::from_wide(&file[..len]))))
}
fn running(game:&Path)->Result<bool>{unsafe{
    let snapshot=Handle(CreateToolhelp32Snapshot(2,0));
    if snapshot.0 as isize==-1 {return Err("无法检查游戏是否已经运行，请退出游戏后重试".into());}
    let mut entry:ProcessEntry=std::mem::zeroed();entry.size=std::mem::size_of::<ProcessEntry>() as u32;
    let mut more=Process32FirstW(snapshot.0,&mut entry)!=0;
    while more {
        // The user may keep this launcher inside the game directory itself.
        let process=Handle(if entry.pid==std::process::id(){ptr::null_mut()}else{OpenProcess(0x1000,0,entry.pid)});
        if !process.0.is_null() {
            let mut buffer=vec![0u16;32768];let mut len=buffer.len() as u32;
            if QueryFullProcessImageNameW(process.0,0,buffer.as_mut_ptr(),&mut len)!=0 {
                let path=PathBuf::from(OsString::from_wide(&buffer[..len as usize]));
                if path.parent().is_some_and(|p|p.as_os_str().to_string_lossy().eq_ignore_ascii_case(&game.as_os_str().to_string_lossy())) {
                    return Ok(true);
                }
            }
        }
        more=Process32NextW(snapshot.0,&mut entry)!=0;
    }
    Ok(false)
}}
fn run()->Result<Option<u32>> {
    let args:Vec<_>=std::env::args_os().skip(1).collect();
    let mut pick=false;let mut game_arg=None;let mut root_arg=None;let mut close_after=5u32;let mut i=0;
    while i<args.len() {
        match args[i].to_str().ok_or("参数包含无效字符")? {
            "--headless"=>{},
            "--help"|"-h"=>{println!("雀魂本地装扮试用 {}\n\n双击 EXE：首次选择游戏，之后使用记住的路径启动并注入同目录 DLL。\n--pick / --choose-game  重新选择游戏\n--game <EXE路径>        指定游戏启动路径\n--close-after <秒>      成功后关闭窗口的倒计时，默认 5；0 保留窗口\n\nEXE 和 {} 必须放在同一个目录。",env!("CARGO_PKG_VERSION"),launcher::DLL_FILE_NAME);return Ok(None);},
            "--pick"|"--choose-game"=>pick=true,
            "--game"=>{i+=1;game_arg=Some(PathBuf::from(args.get(i).ok_or("--game 需要游戏路径")?));},
            "--root"=>{i+=1;root_arg=Some(PathBuf::from(args.get(i).ok_or("--root 需要工具目录")?));},
            "--close-after"=>{i+=1;close_after=args.get(i).and_then(|s|s.to_str()).and_then(|s|s.parse::<u32>().ok()).filter(|v|*v<=3600).ok_or("--close-after 需要 0 到 3600 的秒数")?;},
            other=>return Err(format!("未知参数：{other}；使用 --help 查看用法")),
        }
        i+=1;
    }
    let mutex=Handle(unsafe{CreateMutexW(ptr::null_mut(),0,wide("Local\\MJLocalPreview.Launcher").as_ptr())});
    if mutex.0.is_null() {return Err("无法初始化启动器".into());}
    if unsafe{GetLastError()}==183 {return Err("启动器已经打开，请在原窗口完成操作".into());}
    let root=match root_arg {Some(path)=>{if !path.is_absolute()||!path.is_dir(){return Err("工具目录无效".into());}launcher::unlinked(&path)?;path},None=>std::env::current_exe().map_err(|e|e.to_string())?.parent().ok_or("无法找到启动器目录")?.to_path_buf()};
    let dll=root.join(launcher::DLL_FILE_NAME);injection::validate_dll(&dll)?;
    let settings=root.join("MaJmods.ini");
    let fallback=std::env::var_os("LOCALAPPDATA").map(PathBuf::from).map(|p|p.join(launcher::LEGACY_DATA_DIRECTORY).join("launcher/game-path.txt"));
    let remembered=launcher::load_path(&settings)
        .or_else(||launcher::load_path(&root.join("MJLocalPreview.ini")))
        .or_else(||fallback.as_ref().and_then(|p|launcher::load_path(p)));
    let mut selected=if let Some(exe)=&game_arg {Some(launcher::validate_game(exe)?)} else if pick {None} else {remembered.as_ref().and_then(|p|launcher::validate_game(p).ok())};
    if selected.is_none() && remembered.is_some() && !pick {println!("已保存的游戏路径不可用，请重新选择。");}
    while selected.is_none() {
        let Some(path)=choose(remembered.as_deref())? else {println!("已取消选择。");return Ok(None);};
        if std::env::current_exe().ok().and_then(|p|launcher::normal_path(&p).ok()) == launcher::normal_path(&path).ok() {
            message("请选择雀魂游戏启动程序，不要选择本试用工具。"); continue;
        }
        match launcher::validate_game(&path) {
            Ok(found)=>selected=Some(found),
            Err(error)=>message(&error),
        }
    }
    let (chosen,game)=selected.unwrap();
    println!("已选择：{}",chosen.display());
    match launcher::save_path(&settings,&chosen) {
        Ok(())=>println!("已记住游戏路径，下次直接启动。"),
        Err(e)=>println!("注意：路径无法记住，请将 EXE 和 DLL 放到可写目录：{e}"),
    }
    if running(&game)? {return Err("游戏已经运行，请先退出游戏再运行本工具。已运行的客户端需要重启才能完整加载试用。".into());}
    let data=launcher::data_directory(&root)?;
    bundled::prepare(&data,Some(&game.join(launcher::LEGACY_DATA_DIRECTORY)))?;
    if let Some(archived)=launcher::retire_legacy(&game)? {println!("旧版自动加载文件已移入备份：{}",archived.display());}
    // Selecting an official updater still launches the validated actual game client.
    let exe=if game==chosen.parent().unwrap(){chosen}else{game.join("Jantama_MahjongSoul.exe")};
    let steam_app_id=version::steam::app_id(&exe)?;
    if let Some(id)=steam_app_id {
        println!("已识别 Steam 版（App ID {id}），正在准备游戏启动环境。");
    }
    let mut child=injection::suspended(&exe)?;
    println!("游戏进程已准备：{}\n正在加载同目录 DLL……",child.pid);
    std::io::stdout().flush().ok();
    child.inject(&dll)?;
    println!("DLL 已加载：{}\n[1/2] 本地试用已准备，恢复游戏运行。",dll.display());
    let log=data.join(format!("preview-{}.log",child.pid));println!("本次日志：{}",log.display());
    child.resume()?;
    let end=Instant::now()+Duration::from_secs(25);
    while !child.hooks_ready() {
        if !child.alive(){return Err("游戏在接管确认前已退出，请查看本次日志".into());}
        if Instant::now()>=end {println!("游戏已运行，暂未确认 Lua 接管；请查看本次日志，当前窗口不会自动关闭。{}　",log.display());return Ok(Some(0));}
        std::thread::sleep(Duration::from_millis(100));
    }
    if !child.alive(){return Err("游戏已退出，请查看本次日志".into());}
    if steam_app_id.is_some() {
        println!("正在确认 Steam 客户端保持运行……");
        std::io::stdout().flush().ok();
        if !child.running_after(8000)? {
            return Err("Steam 客户端在启动确认期间退出或重新启动，请查看本次日志并确认 Steam 已登录".into());
        }
    }
    println!("[2/2] Lua 入口已接管。进入大厅后即可在原有页面试用角色、皮肤和装扮。");
    println!("自定义立绘：F10 重载，F11 关闭并恢复原图。\n图片和配置：{}",data.join("mods").display());
    Ok(Some(close_after))
}
fn wait_enter(){println!("按回车关闭本窗口……");let mut line=String::new();let _=std::io::stdin().read_line(&mut line);}
fn main(){
    unsafe{SetConsoleOutputCP(65001);SetConsoleTitleW(wide("雀魂本地装扮试用").as_ptr());}
    let headless=std::env::args_os().any(|a|a=="--headless");
    let mut processes=[0u32;4];let own=!headless && unsafe{GetConsoleProcessList(processes.as_mut_ptr(),4)}==1;
    println!("雀魂本地装扮试用 {}\n",env!("CARGO_PKG_VERSION"));
    match run() {
        Ok(Some(seconds)) if own=>{
            if seconds==0 {wait_enter();} else {
                for left in (1..=seconds).rev(){print!("\r游戏已运行，{left} 秒后关闭本窗口…… ");std::io::stdout().flush().ok();std::thread::sleep(Duration::from_secs(1));}
                println!();
            }
        },
        Ok(_)=>{},
        Err(error)=>{eprintln!("启动失败：{error}");if own {wait_enter();}std::process::exit(1);},
    }
}
