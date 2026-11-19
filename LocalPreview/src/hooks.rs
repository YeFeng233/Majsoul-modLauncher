use std::{cell::Cell, ffi::{c_void,CStr}, fs::OpenOptions, io::Write,
    path::PathBuf, sync::{OnceLock,atomic::{AtomicUsize,AtomicBool,Ordering}}};
use crate::{catalog,transform,win,pe};

struct Data { base:PathBuf, prelude:String, adapter:Vec<transform::Rule>, enabled:bool, dump:bool }
static DATA:OnceLock<Data>=OnceLock::new();
static INSTALLED:AtomicBool=AtomicBool::new(false);
static CLAIMED:AtomicBool=AtomicBool::new(false);
static CAN_PATCH:AtomicBool=AtomicBool::new(false);
static LOAD:AtomicUsize=AtomicUsize::new(0);
static LOADX:AtomicUsize=AtomicUsize::new(0);
static TOLUA:AtomicUsize=AtomicUsize::new(0);
static GETTOP:AtomicUsize=AtomicUsize::new(0);
static SETTOP:AtomicUsize=AtomicUsize::new(0);
static PCALL:AtomicUsize=AtomicUsize::new(0);
static STRING:AtomicUsize=AtomicUsize::new(0);
static PUSH:AtomicUsize=AtomicUsize::new(0);
static FIELD:AtomicUsize=AtomicUsize::new(0);
thread_local! { static BUSY:Cell<bool>=const {Cell::new(false)}; }
type Load=unsafe extern "C" fn(*mut c_void,*const u8,usize,*const i8)->i32;
type LoadX=unsafe extern "C" fn(*mut c_void,*const u8,usize,*const i8,*const i8)->i32;

fn log(message:&str) {
    if let Some(data)=DATA.get() {
        if let Ok(mut file)=OpenOptions::new().create(true).append(true).open(data.base.join(format!("preview-{}.log",std::process::id()))) {
            let _=writeln!(file,"{message}");
        }
    }
}

fn signal(stage:&str) { unsafe {
    let event=win::CreateEventW(std::ptr::null_mut(),1,0,
        win::to_wide(&format!("Local\\MJLocalPreview.{}.{}",std::process::id(),stage)).as_ptr());
    if !event.is_null(){win::SetEvent(event);win::CloseHandle(event);}
} }

pub fn start(instance:*mut c_void) {
    let Some(root)=win::module_dir(instance) else { return; };
    let Ok(base)=crate::launcher::data_directory(&root) else { return; };
    if crate::bundled::prepare(&base,None).is_err() { return; }
    let read=|file:&str|std::fs::read_to_string(base.join(file)).map_err(|e|format!("{file}: {e}"));
    let config=read("config.ini").unwrap_or_default();
    let values:Vec<_>=config.trim_start_matches('\u{feff}').lines().filter_map(|l| l.split('#').next()?.trim().split_once('='))
        .map(|(k,v)|(k.trim(),v.trim())).collect();
    let enabled=values.iter().any(|v|*v==("mode","preview"));
    let dump=values.iter().any(|v|*v==("dump","true"));
    let loaded=(|| {
        let cat=catalog::parse(&read("max_data.yaml")?)?;
        let runtime=read("runtime.lua")?;
        if !runtime.contains("@CATALOG@") { return Err("runtime has no catalogue placeholder".into()); }
        let adapter=transform::rules(&read("adapter.tsv")?)?;
        Ok::<_,String>((runtime.trim_start_matches('\u{feff}').replace("@CATALOG@",&cat),adapter))
    })();
    let (prelude,adapter,error)=match loaded { Ok((p,a))=>(p,a,None),Err(e)=>(String::new(),Vec::new(),Some(e)) };
    let valid=error.is_none();
    let _=DATA.set(Data{base,prelude,adapter,enabled:enabled&&valid,dump});
    log(&format!("\nMJLocalPreview {} x86; pid={}; enabled={}; dump={dump}",env!("CARGO_PKG_VERSION"),std::process::id(),enabled&&valid));
    if let Some(e)=error { log(&format!("DISABLED: {e}")); }
    if !valid { return; }
    if !enabled { signal("ready"); return; }
    crate::custom_skin::initialize(&DATA.get().unwrap().base);
    let mut cookie=std::ptr::null_mut();
    let status=unsafe {win::register_dll_notification(notification,&mut cookie)};
    log(&format!("loader notification status: 0x{status:08x}"));
    if status != 0 { return; }
    signal("ready");
    let name=win::to_wide("tolua.dll");
    for _ in 0..1200 {
        if INSTALLED.load(Ordering::Acquire) { break; }
        let module=unsafe {win::GetModuleHandleW(name.as_ptr())};
        if !module.is_null() { unsafe {install(module as *mut u8)};break; }
        unsafe {win::Sleep(50)};
    }
    log(if INSTALLED.load(Ordering::Acquire) {"Lua loader hooks installed"} else {"ERROR: tolua.dll not found"});
    log(&format!("originals load={:08x} loadx={:08x} tolua={:08x} gettop={:08x} settop={:08x}",
        LOAD.load(Ordering::Acquire),LOADX.load(Ordering::Acquire),TOLUA.load(Ordering::Acquire),
        GETTOP.load(Ordering::Acquire),SETTOP.load(Ordering::Acquire)));
}

extern "system" fn notification(reason:u32, data:*const win::DllLoadedData,_:*mut c_void) {
    if reason!=1 || data.is_null() {return;}
    unsafe {
        let loaded=&*data;
        let Some(name)=loaded.base_dll_name.as_ref() else {return;};
        if name.length!=18 || name.buffer.is_null() {return;}
        let matches=std::slice::from_raw_parts(name.buffer,9).iter().zip(b"tolua.dll")
            .all(|(w,c)|*w==*c as u16 || (*w>=65 && *w<=90 && *w+32==*c as u16));
        if matches {install(loaded.dll_base as *mut u8);}
    }
}

unsafe fn install(base:*mut u8) {
    if CLAIMED.swap(true,Ordering::AcqRel) {return;}
    // Notification runs before callers resolve Lua's entry points. Read its PE
    // tables directly; the loader's module registry need not be ready yet.
    let symbol=|name:&str|pe::export_address(base,name).map_or(0,|p|p as usize);
    GETTOP.store(symbol("lua_gettop"),Ordering::Release);
    SETTOP.store(symbol("lua_settop"),Ordering::Release);
    // Protected calls are the main-thread dispatch point for custom portrait updates.
        STRING.store(symbol("lua_tolstring"),Ordering::Release);
        PCALL.store(symbol("lua_pcall"),Ordering::Release);
        if STRING.load(Ordering::Acquire)!=0 && PCALL.load(Ordering::Acquire)!=0 {
            pe::replace_export(base,"lua_pcall",pcall as *const () as *mut c_void);
        }
    if DATA.get().is_some_and(|d|d.dump) {
        PUSH.store(symbol("lua_pushcclosure"),Ordering::Release);
        FIELD.store(symbol("lua_setfield"),Ordering::Release);
    }
    let mut complete=GETTOP.load(Ordering::Acquire)!=0 && SETTOP.load(Ordering::Acquire)!=0;
    for (name,slot,hook) in [
        ("luaL_loadbuffer",&LOAD,load as *const () as *mut c_void),
        ("luaL_loadbufferx",&LOADX,loadx as *const () as *mut c_void),
        ("tolua_loadbuffer",&TOLUA,tolua as *const () as *mut c_void),
    ] {
        // Publish original address BEFORE export-table mutation; concurrent calls can
        // never see a hook with an unset original pointer.
        slot.store(symbol(name),Ordering::Release);
        complete &= pe::replace_export(base,name,hook).is_some();
    }
    CAN_PATCH.store(complete,Ordering::Release);
    INSTALLED.store(true,Ordering::Release);
    log(if complete {"Lua loader hooks installed"}else{"ERROR: Lua exports are incompatible"});
    if complete {signal("hooks");}
}

unsafe extern "C" fn pcall(l:*mut c_void,args:i32,results:i32,err:i32)->i32 {
    let original:unsafe extern "C" fn(*mut c_void,i32,i32,i32)->i32=std::mem::transmute(PCALL.load(Ordering::Acquire));
    let status=original(l,args,results,err);
    if status!=0 && DATA.get().is_some_and(|d|d.dump) {
        let string:unsafe extern "C" fn(*mut c_void,i32,*mut usize)->*const u8=std::mem::transmute(STRING.load(Ordering::Acquire));
        let mut len=0;let message=string(l,-1,&mut len);
        if !message.is_null() {log(&format!("LUA ERROR {status}: {}",String::from_utf8_lossy(std::slice::from_raw_parts(message,len.min(8192)))));}
    }
    if status==0 { crate::custom_skin::pump(l,PCALL.load(Ordering::Acquire)); }
    status
}

unsafe extern "C" fn trace(l:*mut c_void)->i32 {
    let string:unsafe extern "C" fn(*mut c_void,i32,*mut usize)->*const u8=std::mem::transmute(STRING.load(Ordering::Acquire));
    let mut len=0;let message=string(l,1,&mut len);
    if !message.is_null() {log(&format!("TRACE {}",String::from_utf8_lossy(std::slice::from_raw_parts(message,len.min(4096)))));}
    0
}

unsafe extern "C" fn load(l:*mut c_void,b:*const u8,n:usize,name:*const i8)->i32 {
    dispatch(LOAD.load(Ordering::Acquire),false,l,b,n,name,std::ptr::null())
}
unsafe extern "C" fn loadx(l:*mut c_void,b:*const u8,n:usize,name:*const i8,mode:*const i8)->i32 {
    dispatch(LOADX.load(Ordering::Acquire),true,l,b,n,name,mode)
}
unsafe extern "C" fn tolua(l:*mut c_void,b:*const u8,n:usize,name:*const i8)->i32 {
    dispatch(TOLUA.load(Ordering::Acquire),false,l,b,n,name,std::ptr::null())
}

unsafe fn dispatch(address:usize,is_x:bool,l:*mut c_void,b:*const u8,n:usize,name:*const i8,mode:*const i8)->i32 {
    if address==0 {return 1;}
    let call=|buf:*const u8,len:usize| {
        if is_x { std::mem::transmute::<usize,LoadX>(address)(l,buf,len,name,mode) }
        else {std::mem::transmute::<usize,Load>(address)(l,buf,len,name)}
    };
    if BUSY.with(|v|v.replace(true)) {return call(b,n);}
    struct Reset;impl Drop for Reset {fn drop(&mut self){BUSY.with(|v|v.set(false));}}
    let _reset=Reset;
    let transformed=std::panic::catch_unwind(||prepare(b,n,name)).ok().flatten();
    let Some(out)=transformed else {return call(b,n);};
    let push=PUSH.load(Ordering::Acquire);let field=FIELD.load(Ordering::Acquire);
    if push!=0 && field!=0 {
        let push:unsafe extern "C" fn(*mut c_void,unsafe extern "C" fn(*mut c_void)->i32,i32)=std::mem::transmute(push);
        let field:unsafe extern "C" fn(*mut c_void,i32,*const i8)=std::mem::transmute(field);
        push(l,trace,0);field(l,-10002,c"_MJLocalPreviewTrace".as_ptr());
    }
    let get=GETTOP.load(Ordering::Acquire);let set=SETTOP.load(Ordering::Acquire);
    if get==0 || set==0 {return call(b,n);}
    let get:unsafe extern "C" fn(*mut c_void)->i32=std::mem::transmute(get);
    let set:unsafe extern "C" fn(*mut c_void,i32)=std::mem::transmute(set);
    let top=get(l);let status=call(out.as_ptr(),out.len());
    if status==0 {return 0;}
    log("ERROR: modified chunk failed to compile; restoring original chunk");
    set(l,top);call(b,n)
}

unsafe fn prepare(buffer:*const u8,size:usize,name:*const i8)->Option<Vec<u8>> {
    let data=DATA.get()?;
    if !data.enabled || !CAN_PATCH.load(Ordering::Acquire) || buffer.is_null() || size==0 {return None;}
    let bytes=std::slice::from_raw_parts(buffer,size);
    if bytes.first()==Some(&0x1b) {return None;}
    let source=std::str::from_utf8(bytes).ok()?;
    let chunk=if name.is_null(){"unknown".into()}else{CStr::from_ptr(name).to_string_lossy()};
    match transform::rewrite(source,&data.adapter,&data.prelude) {
        Ok(out) if !out.matched.is_empty()=>{
            log(&format!("PATCH {chunk}: {}",out.matched.join(",")));
            if data.dump {
                let dir=data.base.join("diagnostics");let _=std::fs::create_dir_all(&dir);
                let safe:String=chunk.chars().map(|c|if c.is_ascii_alphanumeric()||c=='_'{c}else{'_'}).collect();
                let _=std::fs::write(dir.join(format!("{safe}.original.lua")),bytes);
                let _=std::fs::write(dir.join(format!("{safe}.patched.lua")),&out.source);
            }
            Some(out.source.into_bytes())
        }
        Ok(_)=>None,
        Err(error)=>{log(&format!("SKIP {chunk}: {error}"));None}
    }
}

