#![allow(non_snake_case)]
pub mod catalog;
pub mod transform;
pub mod launcher;
pub mod injection;
pub mod steam;
pub mod bundled;
mod hooks;
mod pe;
mod proxies;
mod win;
mod custom_skin;

use std::ffi::c_void;

#[no_mangle]
pub extern "system" fn DllMain(instance: *mut c_void, reason: u32, _: *mut c_void) -> i32 {
    if reason == win::DLL_PROCESS_ATTACH {
        unsafe {
            win::DisableThreadLibraryCalls(instance);
            let thread = win::CreateThread(std::ptr::null_mut(), 0, worker,
                instance, 0, std::ptr::null_mut());
            if !thread.is_null() { win::CloseHandle(thread); }
        }
    }
    1
}

extern "system" fn worker(instance: *mut c_void) -> u32 {
    hooks::start(instance);
    0
}

#[no_mangle]
pub extern "system" fn MJLocalPreviewProtocol() -> u32 { 3 }

