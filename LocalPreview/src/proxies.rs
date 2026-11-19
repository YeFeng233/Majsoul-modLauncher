//! `version.dll` proxy.
//!
//! The injector ships AS version.dll: UnityPlayer.dll imports it statically, and Windows
//! resolves a statically imported DLL from the application directory before System32, so
//! our copy is loaded instead of the system one at process start. That gets our code into
//! the process before UnityPlayer loads tolua.dll - with no launcher, no admin rights and
//! no modification of any existing game file.
//!
//! Every export the real DLL offers has to exist here, otherwise the process would fail to
//! start with "entry point not found". Each stub resolves the system function lazily and
//! forwards to it with the exact documented arity (the API is __stdcall on x86, so the
//! callee pops the arguments and an arity mismatch would corrupt the stack).

use core::ffi::c_void;
use std::sync::Mutex;

use crate::win;

static REAL_MODULE: Mutex<usize> = Mutex::new(0);

fn real_fn(name: &str) -> *mut c_void {
    let mut guard = match REAL_MODULE.lock() {
        Ok(guard) => guard,
        Err(_) => return core::ptr::null_mut(),
    };
    if *guard == 0 {
        let path = win::to_wide(&win::system_version_dll());
        let module = unsafe { win::LoadLibraryW(path.as_ptr()) };
        if module.is_null() {
            return core::ptr::null_mut();
        }
        *guard = module as usize;
    }
    let mut cname: Vec<u8> = name.bytes().collect();
    cname.push(0);
    unsafe { win::GetProcAddress(*guard as *mut c_void, cname.as_ptr()) }
}

macro_rules! forward {
    ($name:ident ( $( $arg:ident : $ty:ty ),* ) -> $ret:ty) => {
        #[no_mangle]
        pub unsafe extern "system" fn $name($( $arg : $ty ),*) -> $ret {
            let target = real_fn(stringify!($name));
            if target.is_null() {
                return core::mem::zeroed();
            }
            let function: unsafe extern "system" fn($( $ty ),*) -> $ret = core::mem::transmute(target);
            function($( $arg ),*)
        }
    };
}

// --- the three entry points UnityPlayer actually imports -------------------------------
forward!(GetFileVersionInfoSizeA(filename: *const u8, handle: *mut u32) -> u32);
forward!(GetFileVersionInfoA(filename: *const u8, handle: u32, length: u32, data: *mut c_void) -> i32);
forward!(VerQueryValueA(block: *const c_void, sub_block: *const u8, buffer: *mut *mut c_void, length: *mut u32) -> i32);

// --- the rest of the module's real export set -----------------------------------------
forward!(GetFileVersionInfoSizeW(filename: *const u16, handle: *mut u32) -> u32);
forward!(GetFileVersionInfoSizeExA(flags: u32, filename: *const u8, handle: *mut u32) -> u32);
forward!(GetFileVersionInfoSizeExW(flags: u32, filename: *const u16, handle: *mut u32) -> u32);
forward!(GetFileVersionInfoW(filename: *const u16, handle: u32, length: u32, data: *mut c_void) -> i32);
forward!(GetFileVersionInfoExA(flags: u32, filename: *const u8, handle: u32, length: u32, data: *mut c_void) -> i32);
forward!(GetFileVersionInfoExW(flags: u32, filename: *const u16, handle: u32, length: u32, data: *mut c_void) -> i32);
forward!(GetFileVersionInfoByHandle(flags: u32, file: *mut c_void, handle: u32, length: u32, data: *mut c_void) -> i32);
forward!(VerQueryValueW(block: *const c_void, sub_block: *const u16, buffer: *mut *mut c_void, length: *mut u32) -> i32);
forward!(VerFindFileA(flags: u32, file: *const u8, win_dir: *const u8, app_dir: *const u8, cur_dir: *mut u8, cur_len: *mut u32, dest_dir: *mut u8, dest_len: *mut u32) -> u32);
forward!(VerFindFileW(flags: u32, file: *const u16, win_dir: *const u16, app_dir: *const u16, cur_dir: *mut u16, cur_len: *mut u32, dest_dir: *mut u16, dest_len: *mut u32) -> u32);
forward!(VerInstallFileA(flags: u32, src: *const u8, dest: *const u8, src_dir: *const u8, dest_dir: *const u8, cur_dir: *const u8, tmp: *mut u8, tmp_len: *mut u32) -> u32);
forward!(VerInstallFileW(flags: u32, src: *const u16, dest: *const u16, src_dir: *const u16, dest_dir: *const u16, cur_dir: *const u16, tmp: *mut u16, tmp_len: *mut u32) -> u32);
