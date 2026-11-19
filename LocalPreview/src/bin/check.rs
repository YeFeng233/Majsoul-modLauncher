use std::{collections::BTreeMap,ffi::{c_void,CString},path::{Path,PathBuf}};
use version::{catalog,transform};
#[link(name="kernel32")]
extern "system" {
    fn LoadLibraryW(name:*const u16)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const u8)->*mut c_void;
    fn Sleep(ms:u32);
}
type Load=unsafe extern "C" fn(*mut c_void,*const u8,usize,*const i8)->i32;
struct Lua {
    state:*mut c_void, load:Load,
    pcall:unsafe extern "C" fn(*mut c_void,i32,i32,i32)->i32,
    string:unsafe extern "C" fn(*mut c_void,i32,*mut usize)->*const u8,
    settop:unsafe extern "C" fn(*mut c_void,i32),
    close:unsafe extern "C" fn(*mut c_void),
}
fn wide(path:&Path)->Vec<u16>{path.to_string_lossy().encode_utf16().chain(Some(0)).collect()}
impl Lua {
    unsafe fn open(path:&Path)->Self {
        let dll=LoadLibraryW(wide(path).as_ptr());assert!(!dll.is_null(),"could not load tolua.dll");
        let get=|name:&str| {let c=CString::new(name).unwrap();let p=GetProcAddress(dll,c.as_ptr() as *const u8);assert!(!p.is_null(),"{name}");p};
        let new:unsafe extern "C" fn()->*mut c_void=std::mem::transmute(get("luaL_newstate"));
        let state=new();assert!(!state.is_null());
        let libs:unsafe extern "C" fn(*mut c_void)=std::mem::transmute(get("luaL_openlibs"));libs(state);
        Self{state,load:std::mem::transmute(get("luaL_loadbuffer")),pcall:std::mem::transmute(get("lua_pcall")),
            string:std::mem::transmute(get("lua_tolstring")),settop:std::mem::transmute(get("lua_settop")),close:std::mem::transmute(get("lua_close"))}
    }
    fn check(&self,name:&str,source:&str,execute:bool)->Result<(),String>{unsafe{
        let label=CString::new(name).unwrap();
        let mut status=(self.load)(self.state,source.as_ptr(),source.len(),label.as_ptr());
        if status==0 && execute {status=(self.pcall)(self.state,0,0,0);}
        let result=if status==0{Ok(())}else{
            let mut len=0;let p=(self.string)(self.state,-1,&mut len);
            Err(if p.is_null(){format!("status {status}")}else{String::from_utf8_lossy(std::slice::from_raw_parts(p,len)).into_owned()})
        };
        (self.settop)(self.state,0);result
    }}
}
impl Drop for Lua {fn drop(&mut self){unsafe{(self.close)(self.state);}}}
fn read(path:&Path)->String {std::fs::read_to_string(path).unwrap_or_else(|e|panic!("{}: {e}",path.display()))}
fn main(){
    let args:Vec<_>=std::env::args().skip(1).collect();
    let arg=|name:&str|args.iter().position(|v|v==name).and_then(|i|args.get(i+1)).map(PathBuf::from);
    let tolua=arg("--tolua").expect("--tolua <game tolua.dll>");
    let data=arg("--data").unwrap_or_else(||PathBuf::from("data"));
    let cat=catalog::parse(&read(&data.join("max_data.yaml"))).unwrap();
    let prelude=read(&data.join("runtime.lua")).replace("@CATALOG@",&cat);
    let rules=transform::rules(&read(&data.join("adapter.tsv"))).unwrap();
    if let Some(hook)=arg("--hook") {unsafe{
        assert!(!LoadLibraryW(wide(&hook).as_ptr()).is_null(),"injector failed to load");Sleep(500);
    }}
    let lua=unsafe{Lua::open(&tolua)};
    lua.check("@runtime",&prelude,true).unwrap();
    let mut hits:BTreeMap<String,usize>=BTreeMap::new();let mut count=0;
    let mut definitions=String::new();
    if let Some(dump)=arg("--dump") {
        // The directory may contain stale files from previous sessions. Only the
        // current session index is evidence for compatibility.
        let index=read(&dump.join("index.tsv"));
        for line in index.lines() {
            let file=line.split('\t').next().unwrap_or("");
            if !file.ends_with(".lua") {continue;}
            let path=dump.join(file);
            let src=read(&path);let out=transform::rewrite(&src,&rules,&prelude).unwrap();
            lua.check(&format!("@{}",path.display()),&out.source,false).unwrap();count+=1;
            for key in &out.matched{*hits.entry(key.clone()).or_default()+=1;}
            for name in ["GameMgr:makeCharacterInfo","GameMgr:have_character","GameMgr.SetMainCharacterAndSkin","GameUtility.item_owned","LobbyNetMgr.SendRequest","UI_Bag.FindItemByCategory","UI_Bag.GetCount"] {
                if let Some(body)=transform::function_source(&src,name) {definitions.push_str(body);definitions.push('\n');}
            }
        }
        for rule in &rules {assert_eq!(hits.get(&rule.key).copied().unwrap_or(0),1,"missing/duplicate adapter {}",rule.key);}
        println!("Client syntax: {count} chunks; all {} adapter functions matched exactly once",rules.len());
    }
    if let Some(tests)=arg("--tests") {
        lua.check("@environment",&read(&tests.join("environment.lua")),true).unwrap();
        if !definitions.is_empty(){
            let out=transform::rewrite(&definitions,&rules,&prelude).unwrap();
            lua.check("@actual-client-functions",&out.source,true).unwrap();
        }
        lua.check("@assertions",&read(&tests.join("assertions.lua")),true).unwrap();
        lua.check("@restart-reset","MJLocalTrial=nil",true).unwrap();
        lua.check("@restart-runtime",&prelude,true).unwrap();
        lua.check("@restart-environment",&read(&tests.join("environment.lua")),true).unwrap();
        lua.check("@restart-assert","MJLocalTrial.guard_ready=true;MJLocalTrial.account(GameMgr.Inst);assert(MJLocalTrial.choices.character_id==200002 and MJLocalTrial.skins[200002]==400202,'saved profile lost across runtime restart')",true).unwrap();
        println!("Functional assertions passed (actual client function bodies + runtime)");
        let custom=tests.join("custom_assertions.lua");
        if custom.is_file() {
            let script=read(&data.join("custom_skin.lua")).replace("@MODS@","{{character=200017,skin=0,index=1,options={}}, {character=200017,skin=401701,index=2,options={}}, {character=200018,skin=0,index=3,options={}}}");
            lua.check("@custom-portrait-tests",&format!("MJCustomPortraitSource=[====[{script}]====]\n{}",read(&custom)),true).unwrap();
            println!("Custom portrait ownership, scene lifecycle, reload rollback and F11 restoration passed");
        }
    }
    if arg("--hook").is_some() {
        if args.iter().any(|v|v=="--fallback-only") {
            lua.check("@reset","MJLocalTrial=nil",true).unwrap();
            lua.check("@fallback","LobbyNetMgr={};function LobbyNetMgr.SendRequest(a,b,c,d) ORIGINAL_CALLED=true end;return LobbyNetMgr",true).unwrap();
            lua.check("@fallback-assert","assert(MJLocalTrial==nil);LobbyNetMgr.SendRequest();assert(ORIGINAL_CALLED)",true).unwrap();
            println!("Malformed patch fallback restored the original chunk and Lua stack");
            return;
        }
        // Feed RAW source to the native loader, independently of rewrite() above.
        lua.check("@native-reset","MJLocalTrial=nil",true).unwrap();
        lua.check("@native-test","LobbyNetMgr={}; function LobbyNetMgr.SendRequest(a,b,c,d) error('request escaped') end;return LobbyNetMgr",true).unwrap();
        lua.check("@native-assert","assert(MJLocalTrial and MJLocalTrial.guard_ready,'native source instrumentation missing'); LobbyNetMgr.SendRequest('Lobby','useTitle',{title=600001},function(e,r)assert(not e and r.error.code==0)end)",true).unwrap();
        unsafe {
            let hook=LoadLibraryW(wide(&arg("--hook").unwrap()).as_ptr());
            let system=std::env::var("SystemRoot").unwrap_or_else(|_|"C:\\Windows".into());
            let path=PathBuf::from(&system).join("SysWOW64\\version.dll");
            let dll=LoadLibraryW(wide(&path).as_ptr());
            type Size=unsafe extern "system" fn(*const u8,*mut u32)->u32;
            let proxy:Size=std::mem::transmute(GetProcAddress(hook,b"GetFileVersionInfoSizeA\0".as_ptr()));
            let direct:Size=std::mem::transmute(GetProcAddress(dll,b"GetFileVersionInfoSizeA\0".as_ptr()));
            let name=CString::new(path.to_string_lossy().as_bytes()).unwrap();
            let a=proxy(name.as_ptr() as *const u8,std::ptr::null_mut());
            let b=direct(name.as_ptr() as *const u8,std::ptr::null_mut());
            assert!(a!=0 && a==b,"system version forwarding mismatch");
        }
        // The loadbufferx mode argument must survive proxying (cdecl, five args).
        unsafe{
            let dll=LoadLibraryW(wide(&tolua).as_ptr());
            let f:unsafe extern "C" fn(*mut c_void,*const u8,usize,*const i8,*const i8)->i32=
                std::mem::transmute(GetProcAddress(dll,b"luaL_loadbufferx\0".as_ptr()));
            let text=b"return 1";
            assert_eq!(f(lua.state,text.as_ptr(),text.len(),c"@mode".as_ptr(),c"t".as_ptr()),0);
            (lua.settop)(lua.state,0);
            assert_ne!(f(lua.state,text.as_ptr(),text.len(),c"@mode".as_ptr(),c"b".as_ptr()),0);
            (lua.settop)(lua.state,0);
        }
        println!("Native hook execution, version proxy and loadbufferx mode forwarding passed");
    }
}
