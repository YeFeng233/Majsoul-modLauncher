//! x86 suspended launch + explicit adjacent DLL loading. No code or detection bypass.
use std::{ffi::c_void, path::Path, ptr};
use std::os::windows::ffi::OsStrExt;
use crate::launcher::{self,Result};

fn wide(path:&Path)->Vec<u16>{path.as_os_str().encode_wide().chain(Some(0)).collect()}
fn failure(stage:&str)->String{format!("{stage}：{}",std::io::Error::last_os_error())}
#[repr(C)]
struct Startup {
    size:u32,reserved:*mut u16,desktop:*mut u16,title:*mut u16,
    x:u32,y:u32,width:u32,height:u32,chars_x:u32,chars_y:u32,fill:u32,flags:u32,
    show:u16,reserved_size:u16,reserved_bytes:*mut u8,input:*mut c_void,output:*mut c_void,error:*mut c_void,
}
#[repr(C)]
struct ProcessInfo {process:*mut c_void,thread:*mut c_void,pid:u32,tid:u32}
#[link(name="kernel32")]
extern "system" {
    fn CreateProcessW(app:*const u16,command:*mut u16,process_attrs:*mut c_void,thread_attrs:*mut c_void,
        inherit:i32,flags:u32,env:*mut c_void,cwd:*const u16,start:*mut Startup,info:*mut ProcessInfo)->i32;
    fn CloseHandle(handle:*mut c_void)->i32;
    fn VirtualAllocEx(process:*mut c_void,address:*mut c_void,size:usize,kind:u32,protect:u32)->*mut c_void;
    fn VirtualFreeEx(process:*mut c_void,address:*mut c_void,size:usize,kind:u32)->i32;
    fn WriteProcessMemory(process:*mut c_void,address:*mut c_void,bytes:*const c_void,size:usize,written:*mut usize)->i32;
    fn CreateRemoteThread(process:*mut c_void,attrs:*mut c_void,size:usize,start:*mut c_void,param:*mut c_void,flags:u32,id:*mut u32)->*mut c_void;
    fn WaitForSingleObject(handle:*mut c_void,millis:u32)->u32;
    fn GetExitCodeThread(thread:*mut c_void,code:*mut u32)->i32;
    fn ResumeThread(thread:*mut c_void)->u32;
    fn TerminateProcess(process:*mut c_void,code:u32)->i32;
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const u8)->*mut c_void;
    fn CreateEventW(attrs:*mut c_void,manual:i32,initial:i32,name:*const u16)->*mut c_void;
}
pub struct Handle(*mut c_void);
impl Drop for Handle {fn drop(&mut self){if !self.0.is_null(){unsafe{CloseHandle(self.0);}}}}
pub struct Launched { process:Handle, thread:Handle, ready:Handle, hooks:Handle, pub pid:u32, resumed:bool }
impl Drop for Launched {fn drop(&mut self){if !self.resumed {unsafe{TerminateProcess(self.process.0,1);}}}}
impl Launched {
    pub fn alive(&self)->bool{unsafe{WaitForSingleObject(self.process.0,0)==258}}
    pub fn hooks_ready(&self)->bool{unsafe{WaitForSingleObject(self.hooks.0,0)==0}}
    pub fn running_after(&self, milliseconds:u32)->Result<bool>{
        match unsafe{WaitForSingleObject(self.process.0,milliseconds)} {
            0=>Ok(false),
            258=>Ok(true),
            _=>Err(failure("无法确认游戏运行状态")),
        }
    }
    pub fn resume(&mut self)->Result<()> {
        if unsafe{ResumeThread(self.thread.0)}==u32::MAX {return Err(failure("恢复游戏运行失败"));}
        self.resumed=true;Ok(())
    }
    pub fn inject(&self,dll:&Path)->Result<()> {unsafe {
        let path=wide(dll);let size=path.len()*2;
        let remote=VirtualAllocEx(self.process.0,ptr::null_mut(),size,0x3000,4);
        if remote.is_null(){return Err(failure("无法准备 DLL 路径"));}
        let mut written=0;
        if WriteProcessMemory(self.process.0,remote,path.as_ptr().cast(),size,&mut written)==0 || written!=size {
            let err=failure("写入 DLL 路径失败");VirtualFreeEx(self.process.0,remote,0,0x8000);return Err(err);
        }
        let kernel=GetModuleHandleW(crate::win::to_wide("kernel32.dll").as_ptr());
        let load=GetProcAddress(kernel,c"LoadLibraryW".as_ptr().cast());
        if load.is_null(){VirtualFreeEx(self.process.0,remote,0,0x8000);return Err("找不到系统 DLL 加载入口".into());}
        let worker=Handle(CreateRemoteThread(self.process.0,ptr::null_mut(),0,load,remote,0,ptr::null_mut()));
        if worker.0.is_null(){let err=failure("加载 DLL 失败");VirtualFreeEx(self.process.0,remote,0,0x8000);return Err(err);}
        if WaitForSingleObject(worker.0,30000)!=0 {
            // Leave the path allocation valid until the newly created process is terminated.
            return Err("DLL 加载超时；本次启动的游戏将退出".into());
        }
        let mut module=0;let ok=GetExitCodeThread(worker.0,&mut module)!=0 && module!=0;
        VirtualFreeEx(self.process.0,remote,0,0x8000);
        if !ok{return Err("DLL 加载失败，请确认 EXE 与 DLL 来自同一压缩包".into());}
        // Wait for loader notification registration BEFORE letting Unity resolve Lua APIs.
        if WaitForSingleObject(self.ready.0,10000)!=0 {return Err("DLL 已加载，但本地试用初始化未完成；请检查本次日志".into());}
        Ok(())
    }}
}
pub fn validate_dll(dll:&Path)->Result<()> {
    launcher::unlinked(dll)?;
    if !dll.is_file() {return Err(format!("找不到同目录的 {}，请将压缩包完整解压后运行 EXE",launcher::DLL_FILE_NAME));}
    if launcher::pe_machine(dll)?!=0x14c{return Err("DLL 位数不匹配，需要本工具的 32 位 DLL".into());}
    let bytes=std::fs::read(dll).map_err(|e|e.to_string())?;
    if !bytes.windows(b"MJLocalPreviewProtocol".len()).any(|p|p==b"MJLocalPreviewProtocol") {
        return Err("旁边的 DLL 不是兼容的本地试用模块，请重新解压完整压缩包".into());
    }
    Ok(())
}
pub fn suspended(exe:&Path)->Result<Launched>{unsafe {
    let mut environment=crate::steam::environment(exe)?;
    let environment_pointer=environment.as_mut().map_or(ptr::null_mut(),|block|block.as_mut_ptr().cast());
    let flags=4|8|if environment.is_some(){0x400}else{0};
    let mut start:Startup=std::mem::zeroed();start.size=std::mem::size_of::<Startup>() as u32;
    let mut info:ProcessInfo=std::mem::zeroed();
    let mut command=crate::win::to_wide(&format!("\"{}\"",exe.display()));let app=wide(exe);let cwd=wide(exe.parent().ok_or("无效游戏目录")?);
    if CreateProcessW(app.as_ptr(),command.as_mut_ptr(),ptr::null_mut(),ptr::null_mut(),0,flags,environment_pointer,cwd.as_ptr(),&mut start,&mut info)==0 {
        return Err(failure("启动游戏失败"));
    }
    let event=|stage:&str|Handle(CreateEventW(ptr::null_mut(),1,0,crate::win::to_wide(&format!("Local\\MJLocalPreview.{}.{}",info.pid,stage)).as_ptr()));
    let launched=Launched{process:Handle(info.process),thread:Handle(info.thread),ready:event("ready"),hooks:event("hooks"),pid:info.pid,resumed:false};
    if launched.ready.0.is_null() || launched.hooks.0.is_null(){return Err(failure("无法创建启动状态事件"));}
    Ok(launched)
}}
