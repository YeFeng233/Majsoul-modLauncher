//! Character/skin mod discovery and transactional main-thread portrait updates.
use std::{ffi::c_void,sync::{Arc,Mutex,OnceLock,atomic::{AtomicU32,Ordering}},cell::{Cell,RefCell},path::{Path,PathBuf},io::Write,collections::{BTreeMap,BTreeSet}};
static ROOT:OnceLock<PathBuf>=OnceLock::new();
static SNAPSHOT:Mutex<Option<Arc<Vec<Mod>>>>=Mutex::new(None);
static COMMAND:AtomicU32=AtomicU32::new(0);
static LAST:AtomicU32=AtomicU32::new(0);
thread_local!{static BUSY:Cell<bool>=const{Cell::new(false)};static ACTIVE:RefCell<Option<Arc<Vec<Mod>>>>=const{RefCell::new(None)};}
#[derive(Debug)]
struct Mod{character:u32,skin:u32,options:BTreeMap<String,f64>,pixels:Vec<u8>,spine:Vec<SpineTexture>,smallhead:Option<Vec<u8>>,bighead:Option<Vec<u8>>}
impl Mod{
 fn images(&self)->impl Iterator<Item=&Vec<u8>>{
  std::iter::once(&self.pixels).chain(self.spine.iter().map(|t|&t.pixels)).chain(self.smallhead.iter()).chain(self.bighead.iter())
 }
}
fn avatar_image(folder:&Path,name:&str)->Result<Option<Vec<u8>>,String>{
 // Launcher-written English names take priority over imported Chinese names.
 let alias=match name{"smallhead"=>"小头像","bighead"=>"大头像",_=>return Err(format!("Unknown avatar kind: {name}"))};
 for filename in [format!("{name}.png"),format!("{alias}.png")]{
  let path=folder.join(filename);crate::launcher::unlinked(&path)?;
  if path.exists(){return decode_image(&path).map(Some);}
 }
 Ok(None)
}
#[derive(Debug)]
struct SpineTexture{layer:u32,name:String,pixels:Vec<u8>}
fn spine_textures(folder:&Path)->Result<Vec<SpineTexture>,String>{
 let root=folder.join("spine");crate::launcher::unlinked(&root)?;
 if !root.exists(){return Ok(Vec::new());}
 let mut textures=Vec::new();
 for directory in directories(&root)?{
  let label=directory.file_name().and_then(|s|s.to_str()).ok_or("Invalid Spine layer name")?;
  let layer=label.strip_prefix("layer").and_then(|s|s.parse::<u32>().ok()).filter(|n|*n>0&&*n<=32).ok_or("Spine folders must be layer1 through layer32")?;
  for entry in std::fs::read_dir(&directory).map_err(error)?{
   let path=entry.map_err(error)?.path();crate::launcher::unlinked(&path)?;
   if !path.extension().is_some_and(|s|s.eq_ignore_ascii_case("png")){continue;}
   let name=path.file_stem().and_then(|s|s.to_str()).ok_or("Invalid Spine texture name")?.to_string();
   if !name.bytes().all(|c|c.is_ascii_alphanumeric()||matches!(c,b'_'|b'-')){return Err("Spine texture names must contain ASCII letters, digits, underscores or hyphens".into());}
   if textures.iter().any(|t:&SpineTexture|t.layer==layer){return Err("Only one atlas page per Spine layer is currently supported".into());}
   textures.push(SpineTexture{layer,name,pixels:decode_image(&path)?});
  }
 }
 textures.sort_by_key(|t|t.layer);Ok(textures)
}
type Pcall=unsafe extern "C" fn(*mut c_void,i32,i32,i32)->i32;
fn log(s:&str){if let Some(p)=ROOT.get(){if let Ok(mut f)=std::fs::OpenOptions::new().create(true).append(true).open(p.join(format!("custom-skin-{}.log",std::process::id()))){let _=writeln!(f,"{s}");}}}
#[link(name="kernel32")] extern "system"{fn GetTickCount()->u32;}
#[link(name="user32")] extern "system"{fn GetAsyncKeyState(k:i32)->i16;fn GetForegroundWindow()->*mut c_void;fn GetWindowThreadProcessId(h:*mut c_void,p:*mut u32)->u32;}
unsafe fn api(name:&[u8])->usize{let m=crate::win::GetModuleHandleW(crate::win::to_wide("tolua.dll").as_ptr());crate::win::GetProcAddress(m,name.as_ptr())as usize}
fn error(e:impl std::fmt::Display)->String{e.to_string()}
fn directories(root:&Path)->Result<Vec<PathBuf>,String>{
 crate::launcher::unlinked(root)?;let mut dirs=Vec::new();
 for entry in std::fs::read_dir(root).map_err(error)?{let path=entry.map_err(error)?.path();crate::launcher::unlinked(&path)?;if path.is_dir(){dirs.push(path);}}
 dirs.sort();Ok(dirs)
}
fn config(path:&Path)->Result<BTreeMap<String,String>,String>{
 crate::launcher::unlinked(path)?;let bytes=std::fs::read(path).map_err(error)?;if bytes.len()>65536{return Err("mod.ini exceeds 64 KiB".into());}
 let text=std::str::from_utf8(&bytes).map_err(|_|"mod.ini must use UTF-8")?;let mut values=BTreeMap::new();
 for (index,line) in text.trim_start_matches('\u{feff}').lines().enumerate(){let line=line.trim();if line.is_empty()||line.starts_with(['#',';']){continue;}
  let(k,v)=line.split_once('=').ok_or_else(||format!("Invalid mod.ini line {}",index+1))?;
  if values.insert(k.trim().to_string(),v.trim().to_string()).is_some(){return Err(format!("Duplicate setting: {}",k.trim()));}
 }Ok(values)
}
fn integer(values:&BTreeMap<String,String>,key:&str,alias:&str,default:Option<u32>)->Result<u32,String>{
 match (values.get(key),values.get(alias)){
  (Some(_),Some(_))=>Err(format!("Use only {key}, not both {key} and {alias}")),
  (Some(v),None)|(None,Some(v))=>v.parse().map_err(|_|format!("Invalid numeric ID: {key}")),
  _=>default.ok_or_else(||format!("Missing {key}")),
 }
}
fn scan(root:&Path)->Result<Vec<Mod>,String>{
 if !root.join("mods").exists(){return Ok(Vec::new());}
 let mut mods=Vec::new();let mut keys=BTreeSet::new();let mut total=0;
 for character in directories(&root.join("mods"))?{for folder in directories(&character)?{
  let ini=folder.join("mod.ini");if !ini.is_file(){continue;}
  let result=(||{
   let values=config(&ini)?;
   let enabled=values.get("enabled").map(|s|s.parse::<u32>()).transpose().map_err(|_|"Invalid enabled")?.unwrap_or(1);
   if enabled>1{return Err("enabled must be 0 or 1".into());}if enabled==0{return Ok(None);}
   let character=integer(&values,"character_id","character",None)?;if character==0{return Err("character_id must be positive".into());}
   let skin=integer(&values,"skin_id","skin",Some(0))?;
   if !keys.insert((character,skin)){return Err(format!("Duplicate enabled mod for character {character}, skin {skin}"));}
   let image=PathBuf::from(values.get("image").map(String::as_str).unwrap_or("custom.png"));
   if image.is_absolute()||image.components().any(|c|!matches!(c,std::path::Component::Normal(_)|std::path::Component::CurDir)){return Err("image must be a relative path inside this mod".into());}
   let mut path=folder.clone();for part in image.components(){path.push(part);crate::launcher::unlinked(&path)?;}
   if !path.extension().is_some_and(|v|v.eq_ignore_ascii_case("png")){return Err("image must be PNG".into());}
   let mut options=BTreeMap::new();
   for (key,value) in &values{
    if matches!(key.as_str(),"enabled"|"character_id"|"character"|"skin_id"|"skin"|"image"){continue;}
    let valid=["lobby","room","match","ending","dorm","dorm_preview"].iter().any(|scene|["x","y","height"].iter().any(|axis|*key==format!("{scene}_{axis}")));
    if !valid{return Err(format!("Unknown mod setting: {key}"));}
    let number=value.parse::<f64>().map_err(|_|format!("Invalid layout value: {key}"))?;
    if !number.is_finite()||number.abs()>100000.0||(key.ends_with("_height")&&number<=0.0){return Err(format!("Invalid layout value: {key}"));}
    options.insert(key.clone(),number);
   }
   let pixels=decode_image(&path)?;let spine=spine_textures(&folder)?;
   let smallhead=avatar_image(&folder,"smallhead")?;
   let bighead=avatar_image(&folder,"bighead")?;
   let entry=Mod{character,skin,options,pixels,spine,smallhead,bighead};
   total+=entry.images().map(Vec::len).sum::<usize>();
   if total>1024*1024*1024{return Err("Total decoded mod images exceed 1024 MiB".into());}
   Ok(Some(entry))
  })();
  match result{Ok(Some(m))=>mods.push(m),Ok(None)=>{},Err(e)=>return Err(format!("{}: {e}",ini.display()))}
 }}Ok(mods)
}
fn prepare(base:&Path)->Result<(),String>{
 let root=base.join("mods");crate::launcher::unlinked(&root)?;
 if !root.exists(){
  // One-time import of the previous image/config; never recreate a deliberately empty mods folder.
  let legacy=base.join("CustomSkins");let oldini=legacy.join("custom_skin.ini");let oldimage=legacy.join("custom.png");
  let mut text=String::from_utf8(include_bytes!("../data/custom_skin.ini").to_vec()).unwrap();
  let mut image=include_bytes!("../data/custom.png").to_vec();
  crate::launcher::unlinked(&legacy)?;
  if oldini.is_file(){crate::launcher::unlinked(&oldini)?;text=std::fs::read_to_string(&oldini).map_err(error)?;}
  if oldimage.is_file(){crate::launcher::unlinked(&oldimage)?;image=std::fs::read(&oldimage).map_err(error)?;}
  let folder=root.join("二之宫花").join("默认替换");std::fs::create_dir_all(&folder).map_err(error)?;
  crate::launcher::atomic_write(&folder.join("custom.png"),&image)?;
  crate::launcher::atomic_write(&folder.join("mod.ini"),format!("# IDs determine matching; folder names are labels.\nimage = custom.png\n{text}").as_bytes())?;
 }
 crate::launcher::atomic_write(&base.join("custom_skin.lua"),include_bytes!("../data/custom_skin.lua"))?;Ok(())
}
fn reload(){match scan(ROOT.get().unwrap()){
 Ok(mods)=>{log(&format!("Loaded {} enabled mods ({} small avatars, {} large avatars)",mods.len(),mods.iter().filter(|m|m.smallhead.is_some()).count(),mods.iter().filter(|m|m.bighead.is_some()).count()));*SNAPSHOT.lock().unwrap()=Some(Arc::new(mods));COMMAND.store(1,Ordering::Release);}
 Err(e)=>log(&format!("Reload rejected; current images retained: {e}")),
}}
pub fn initialize(base:&Path){
 if ROOT.set(base.to_path_buf()).is_err(){return;}if let Err(e)=prepare(base){log(&e);return;}
 std::thread::spawn(||{reload();unsafe{
 let(mut a0,mut b0)=(false,false);loop{
  let mut owner=0;GetWindowThreadProcessId(GetForegroundWindow(),&mut owner);
  let a=GetAsyncKeyState(0x79);let b=GetAsyncKeyState(0x7a);
  if owner==std::process::id(){if(a<0&&!a0)||a&1!=0{reload();}if(b<0&&!b0)||b&1!=0{COMMAND.store(2,Ordering::Release);log("F11 close queued");}}
  let command=base_path("command.txt");if let Ok(text)=std::fs::read_to_string(&command){let _=std::fs::remove_file(command);if text.trim()=="close"{COMMAND.store(2,Ordering::Release);}else{reload();}}
  a0=a<0;b0=b<0;crate::win::Sleep(20);
 }
 }});
}
fn base_path(name:&str)->PathBuf{ROOT.get().unwrap().join(name)}
fn manifest(mods:&[Mod])->String{
 let mut out=String::from("{");let mut index=1;
 for m in mods{
  out.push_str(&format!("{{character={},skin={},index={},options={{",m.character,m.skin,index));index+=1;
  for(k,v)in &m.options{out.push_str(&format!("{k}={v},"));}
  out.push_str("},spine={");
  for t in &m.spine{out.push_str(&format!("{{layer={},name='{}',index={}}},",t.layer,t.name,index));index+=1;}
  out.push_str("},");
  for (name,pixels) in [("smallhead",&m.smallhead),("bighead",&m.bighead)]{
   if pixels.is_some(){out.push_str(&format!("{name}={{index={index}}},"));index+=1;}
  }
  out.push_str("},");
 }
 out.push('}');out
}
unsafe extern "C" fn pixels(l:*mut c_void)->i32{
 let index:unsafe extern "C" fn(*mut c_void,i32)->isize=std::mem::transmute(api(b"lua_tointeger\0"));
 let push:unsafe extern "C" fn(*mut c_void,*const u8,usize)=std::mem::transmute(api(b"lua_pushlstring\0"));let i=index(l,1);
 ACTIVE.with(|snapshot|{
  let snapshot=snapshot.borrow();
  let pixels=snapshot.as_ref().and_then(|mods|i.checked_sub(1).and_then(|index|mods.iter().flat_map(Mod::images).nth(index as usize)));
  if let Some(bytes)=pixels{push(l,bytes.as_ptr(),bytes.len());}else{push(l,b"".as_ptr(),0);}
 });1
}
pub fn pump(l:*mut c_void,original:usize){unsafe{
 if ROOT.get().is_none()||original==0{return;}if BUSY.with(|b|b.replace(true)){return;}
 struct Guard;impl Drop for Guard{fn drop(&mut self){ACTIVE.with(|a|*a.borrow_mut()=None);BUSY.with(|b|b.set(false));}}let _guard=Guard;
 let now=GetTickCount();if now.wrapping_sub(LAST.load(Ordering::Relaxed))<80{return;}LAST.store(now,Ordering::Relaxed);
 let cmd=COMMAND.swap(0,Ordering::AcqRel);
 let source=(||->Result<String,String>{
  if cmd==2{return Ok("if MJCustomPortrait then MJCustomPortrait.close() end;return true".into());}
  if cmd==0{return Ok("if MJCustomPortrait then MJCustomPortrait.update() end;return true".into());}
  let snapshot=SNAPSHOT.lock().unwrap().clone().ok_or("No mod snapshot")?;
  let text=std::fs::read_to_string(base_path("custom_skin.lua")).map_err(error)?;
  let source=text.replace("@MODS@",&manifest(&snapshot));ACTIVE.with(|a|*a.borrow_mut()=Some(snapshot));
  Ok(format!("if not(GameMgr and GameMgr.Inst and GameMgr.Inst.account_id and UI_Character_Skin)then return false end;\n{source}"))
 })();let text=match source{Ok(s)=>s,Err(e)=>{log(&e);return;}};
 let get:unsafe extern "C" fn(*mut c_void)->i32=std::mem::transmute(api(b"lua_gettop\0"));let set:unsafe extern "C" fn(*mut c_void,i32)=std::mem::transmute(api(b"lua_settop\0"));
 let boolean:unsafe extern "C" fn(*mut c_void,i32)->i32=std::mem::transmute(api(b"lua_toboolean\0"));let load:unsafe extern "C" fn(*mut c_void,*const u8,usize,*const i8)->i32=std::mem::transmute(api(b"luaL_loadbuffer\0"));
 let top=get(l);
 if cmd==1{let push:unsafe extern "C" fn(*mut c_void,unsafe extern "C" fn(*mut c_void)->i32,i32)=std::mem::transmute(api(b"lua_pushcclosure\0"));let field:unsafe extern "C" fn(*mut c_void,i32,*const i8)=std::mem::transmute(api(b"lua_setfield\0"));push(l,pixels,0);field(l,-10002,c"_MJCustomPixels".as_ptr());}
 let mut status=load(l,text.as_ptr(),text.len(),c"@MJCustomPortrait".as_ptr());if status==0{status=std::mem::transmute::<usize,Pcall>(original)(l,0,1,0);}
 if status!=0{let string:unsafe extern "C" fn(*mut c_void,i32,*mut usize)->*const u8=std::mem::transmute(api(b"lua_tolstring\0"));let mut len=0;let p=string(l,-1,&mut len);if !p.is_null(){log(&String::from_utf8_lossy(std::slice::from_raw_parts(p,len.min(4096))));}}
 else if cmd==1&&boolean(l,-1)==0{let _=COMMAND.compare_exchange(0,1,Ordering::AcqRel,Ordering::Relaxed);}
 else if cmd!=0{log(&format!("mod command {cmd} applied"));}set(l,top);
}}

#[repr(C)]struct GdiInput{version:u32,callback:usize,background:i32,external:i32}
#[repr(C)]struct Rect{x:i32,y:i32,width:i32,height:i32}
#[repr(C)]struct BitmapData{width:u32,height:u32,stride:i32,format:i32,scan:*mut u8,reserved:usize}
#[link(name="gdiplus")]
extern "system"{
 fn GdiplusStartup(token:*mut usize,input:*const GdiInput,output:*mut c_void)->i32;
 fn GdiplusShutdown(token:usize);
 fn GdipCreateBitmapFromFile(path:*const u16,image:*mut *mut c_void)->i32;
 fn GdipGetImageWidth(image:*mut c_void,width:*mut u32)->i32;
 fn GdipGetImageHeight(image:*mut c_void,height:*mut u32)->i32;
 fn GdipBitmapLockBits(image:*mut c_void,rect:*const Rect,flags:u32,format:i32,data:*mut BitmapData)->i32;
 fn GdipBitmapUnlockBits(image:*mut c_void,data:*mut BitmapData)->i32;
 fn GdipDisposeImage(image:*mut c_void)->i32;
}
fn decode_image(image:&Path)->Result<Vec<u8>,String>{unsafe{
 let input=GdiInput{version:1,callback:0,background:0,external:0};let mut token=0;
 if GdiplusStartup(&mut token,&input,std::ptr::null_mut())!=0{return Err("PNG decoder initialization failed".into());}
 let mut bitmap=std::ptr::null_mut();let path=crate::win::to_wide(&image.to_string_lossy());
 if GdipCreateBitmapFromFile(path.as_ptr(),&mut bitmap)!=0{GdiplusShutdown(token);return Err(format!("Unable to decode image: {}",image.display()));}
 let result=(||{
 let(mut w,mut h)=(0,0);GdipGetImageWidth(bitmap,&mut w);GdipGetImageHeight(bitmap,&mut h);
 if w==0||h==0||w>4096||h>8192{return Err("PNG dimensions must be within 4096 x 8192".into());}
 let rect=Rect{x:0,y:0,width:w as i32,height:h as i32};let mut data:BitmapData=std::mem::zeroed();
 if GdipBitmapLockBits(bitmap,&rect,1,0x26200a,&mut data)!=0{return Err("PNG pixel decoding failed".into());}
 let mut rgba=Vec::with_capacity(8+(w*h*4)as usize);rgba.extend_from_slice(&w.to_le_bytes());rgba.extend_from_slice(&h.to_le_bytes());
 for y in (0..h).rev(){let row=data.scan.offset(y as isize*data.stride as isize);for x in 0..w{let p=row.add((x*4)as usize);rgba.extend_from_slice(&[*p.add(2),*p.add(1),*p,*p.add(3)]);}}
 GdipBitmapUnlockBits(bitmap,&mut data);
 Ok(rgba)
 })();GdipDisposeImage(bitmap);GdiplusShutdown(token);result
}}

#[cfg(test)]mod tests{
 use super::*;
 struct Temp(PathBuf);impl Temp{fn new()->Self{let base=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("mods-test-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));std::fs::create_dir_all(&base).unwrap();Self(base)}
 fn write(&self,folder:&str,text:&str,image:bool)->PathBuf{let dir=self.0.join("mods").join(folder);std::fs::create_dir_all(&dir).unwrap();std::fs::write(dir.join("mod.ini"),text).unwrap();if image{std::fs::write(dir.join("custom.png"),include_bytes!("../data/custom.png")).unwrap();}dir}}
 impl Drop for Temp{fn drop(&mut self){assert!(self.0.starts_with(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target")));let _=std::fs::remove_dir_all(&self.0);}}
 #[test]fn unicode_mods_and_separate_ids_decode_and_reload(){
  let root=Temp::new();let folder=root.write("二之宫花/原皮肤","\u{feff}enabled=1\ncharacter_id=200017\nskin_id=401701\nimage=立绘.png\ndorm_x=-12.5\n",false);
  std::fs::write(folder.join("立绘.png"),include_bytes!("../data/custom.png")).unwrap();
  root.write("二之宫花/通用","character_id=200017\nskin_id=0",true);
  root.write("另一角色/冬装","character=200018\nskin=401801",true);
  root.write("另一角色/停用","enabled=0\nimage=missing.png",false);
  let first=scan(&root.0).unwrap();assert_eq!(first.len(),3);let exact=first.iter().find(|m|m.skin==401701).unwrap();assert_eq!(exact.character,200017);assert_eq!(exact.options["dorm_x"],-12.5);
  assert_eq!(u32::from_le_bytes(exact.pixels[0..4].try_into().unwrap()),416);assert_eq!(exact.pixels.len(),8+416*1197*4);
  // A new configuration is discovered; previously captured data is immutable.
  std::fs::write(folder.join("mod.ini"),"enabled=0").unwrap();assert_eq!(scan(&root.0).unwrap().len(),2);assert_eq!(first.len(),3);
  std::fs::write(root.0.join("mods/二之宫花/通用/mod.ini"),"enabled=0").unwrap();std::fs::write(root.0.join("mods/另一角色/冬装/mod.ini"),"enabled=0").unwrap();assert!(scan(&root.0).unwrap().is_empty());
 }
 #[test]fn rejects_ambiguous_ids_invalid_config_and_escaping_images(){
  let root=Temp::new();let folder=root.write("角色/测试","character_id=200017",true);
  for config in ["character_id=nope","character_id=200017\nskin_id=-1","character_id=200017\nimage=../custom.png","character_id=200017\nimage=C:\\custom.png","character_id=200017\ndorm_height=0","character_id=200017\ndorm_x=NaN","character_id=200017\nunknown=1","character_id=200017\ncharacter=200017"]{
   std::fs::write(folder.join("mod.ini"),config).unwrap();assert!(scan(&root.0).is_err(),"accepted {config}");
  }
  std::fs::write(folder.join("mod.ini"),"character_id=200017").unwrap();root.write("角色/冲突","character_id=200017",true);assert!(scan(&root.0).unwrap_err().contains("Duplicate enabled mod"));
  std::fs::write(root.0.join("mods/角色/冲突/mod.ini"),"enabled=0").unwrap();std::fs::write(folder.join("custom.png"),b"corrupt PNG").unwrap();assert!(scan(&root.0).unwrap_err().contains("Unable to decode"));
 }
 #[test]fn loads_spine_pages_and_rejects_ambiguous_layers(){
  let root=Temp::new();let folder=root.write("角色/动态","character_id=200017\nskin_id=401701",true);
  let layer=folder.join("spine/layer2");std::fs::create_dir_all(&layer).unwrap();
  std::fs::write(layer.join("test_atlas.png"),include_bytes!("../data/custom.png")).unwrap();
  let mods=scan(&root.0).unwrap();let page=&mods[0].spine[0];
  assert_eq!(page.layer,2);assert_eq!(page.name,"test_atlas");assert_eq!(page.pixels,mods[0].pixels);
  let manifest=manifest(&mods);assert!(manifest.contains("name='test_atlas',index=2"));
  std::fs::write(layer.join("second.png"),include_bytes!("../data/custom.png")).unwrap();
  assert!(scan(&root.0).unwrap_err().contains("Only one atlas page"));
 }
 #[test]fn optional_avatar_images_share_manifest_pixel_order(){
  let root=Temp::new();let folder=root.write("角色/头像","character_id=200017\nskin_id=401701",true);
  let initial=scan(&root.0).unwrap();assert!(initial[0].smallhead.is_none()&&initial[0].bighead.is_none());
  for name in ["smallhead.png","bighead.png"]{std::fs::write(folder.join(name),include_bytes!("../data/custom.png")).unwrap();}
  let layer=folder.join("spine/layer1");std::fs::create_dir_all(&layer).unwrap();
  std::fs::write(layer.join("test_atlas.png"),include_bytes!("../data/custom.png")).unwrap();
  let mods=scan(&root.0).unwrap();let entry=&mods[0];
  assert_eq!(entry.smallhead.as_ref().unwrap(),&entry.pixels);assert_eq!(entry.bighead.as_ref().unwrap(),&entry.pixels);
  assert_eq!(entry.images().count(),4);
  let text=manifest(&mods);assert!(text.contains("name='test_atlas',index=2")&&text.contains("smallhead={index=3}")&&text.contains("bighead={index=4}"));
  std::fs::write(folder.join("smallhead.png"),b"broken avatar").unwrap();
  assert!(scan(&root.0).unwrap_err().contains("Unable to decode"));
  assert_eq!(entry.images().count(),4);
 }
 #[test]fn imported_chinese_avatars_load_and_english_names_take_priority(){
  let root=Temp::new();let folder=root.write("福姬/契约","character_id=200038\nskin_id=403802",true);
  for name in ["小头像.png","大头像.png"]{std::fs::write(folder.join(name),include_bytes!("../data/custom.png")).unwrap();}
  let mods=scan(&root.0).unwrap();let entry=&mods[0];
  assert_eq!(entry.smallhead.as_ref().unwrap(),&entry.pixels);assert_eq!(entry.bighead.as_ref().unwrap(),&entry.pixels);
  assert!(manifest(&mods).contains("smallhead={index=2},bighead={index=3}"));
  // An invalid selected alias aborts reload, preserving the prior snapshot.
  std::fs::write(folder.join("小头像.png"),b"broken imported avatar").unwrap();
  assert!(scan(&root.0).unwrap_err().contains("Unable to decode"));
  std::fs::write(folder.join("smallhead.png"),include_bytes!("../data/custom.png")).unwrap();
  assert_eq!(scan(&root.0).unwrap()[0].smallhead,entry.smallhead);
  // Do not silently fall back to the alias when the English override is bad.
  std::fs::write(folder.join("bighead.png"),b"broken override").unwrap();
  assert!(scan(&root.0).unwrap_err().contains("Unable to decode"));
 }
 #[test]fn imports_old_skin_once_and_preserves_user_changes(){
  let root=Temp::new();let legacy=root.0.join("CustomSkins");std::fs::create_dir_all(&legacy).unwrap();std::fs::write(legacy.join("custom_skin.ini"),"character=200017\nskin=401701\nlobby_y=321").unwrap();std::fs::write(legacy.join("custom.png"),include_bytes!("../data/custom.png")).unwrap();
  prepare(&root.0).unwrap();let mods=scan(&root.0).unwrap();assert_eq!(mods[0].skin,401701);assert_eq!(mods[0].options["lobby_y"],321.0);
  let ini=root.0.join("mods/二之宫花/默认替换/mod.ini");std::fs::write(&ini,"enabled=0").unwrap();prepare(&root.0).unwrap();assert!(scan(&root.0).unwrap().is_empty());assert_eq!(std::fs::read_to_string(ini).unwrap(),"enabled=0");assert!(legacy.join("custom.png").is_file());
 }
}
