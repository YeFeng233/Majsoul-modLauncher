//! Minimal Win32 / ntdll bindings. Declared by hand so the crate needs no dependencies
//! and builds offline for i686-pc-windows-msvc.
#![allow(non_snake_case)]

use core::ffi::c_void;

pub const DLL_PROCESS_ATTACH: u32 = 1;
pub const PAGE_READWRITE: u32 = 0x04;

#[link(name = "kernel32")]
extern "system" {
    pub fn CloseHandle(handle: *mut c_void) -> i32;
    pub fn DisableThreadLibraryCalls(module: *mut c_void) -> i32;
    pub fn CreateThread(
        attributes: *mut c_void,
        stack_size: usize,
        start: extern "system" fn(*mut c_void) -> u32,
        parameter: *mut c_void,
        flags: u32,
        thread_id: *mut u32,
    ) -> *mut c_void;
    pub fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    pub fn LoadLibraryW(name: *const u16) -> *mut c_void;
    pub fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    pub fn VirtualProtect(address: *mut c_void, size: usize, new_protect: u32, old: *mut u32) -> i32;
    pub fn GetModuleFileNameW(module: *mut c_void, buffer: *mut u16, size: u32) -> u32;
    pub fn Sleep(milliseconds: u32);
    pub fn CreateEventW(attributes: *mut c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
    pub fn SetEvent(event: *mut c_void) -> i32;
}

pub type DllNotificationFn = extern "system" fn(u32, *const DllLoadedData, *mut c_void);
type LdrRegisterFn = unsafe extern "system" fn(u32, DllNotificationFn, *mut c_void, *mut *mut c_void) -> i32;

/// `LdrRegisterDllNotification` lives in ntdll but is absent from the MSVC import library,
/// so it is resolved by hand. Returns a non-zero NTSTATUS on failure.
pub unsafe fn register_dll_notification(callback: DllNotificationFn, cookie: *mut *mut c_void) -> i32 {
    let ntdll_name = to_wide("ntdll.dll");
    let ntdll = GetModuleHandleW(ntdll_name.as_ptr());
    if ntdll.is_null() {
        return -1;
    }
    let symbol = GetProcAddress(ntdll, c"LdrRegisterDllNotification".as_ptr() as *const u8);
    if symbol.is_null() {
        return -1;
    }
    let register: LdrRegisterFn = core::mem::transmute(symbol);
    register(0, callback, core::ptr::null_mut(), cookie)
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct UnicodeString {
    pub length: u16,
    pub maximum_length: u16,
    pub buffer: *mut u16,
}

#[repr(C)]
pub struct DllLoadedData {
    pub flags: u32,
    pub full_dll_name: *const UnicodeString,
    pub base_dll_name: *const UnicodeString,
    pub dll_base: *mut c_void,
    pub size_of_image: u32,
}

/// NUL-terminated UTF-16 for the W APIs.
pub fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(core::iter::once(0)).collect()
}

/// Directory holding the running executable - where the injector looks for its data files.
pub fn exe_dir() -> Option<std::path::PathBuf> {
    module_dir(core::ptr::null_mut())
}

pub fn module_dir(module: *mut c_void) -> Option<std::path::PathBuf> {
    let mut buffer = vec![0u16; 32768];
    let written = unsafe { GetModuleFileNameW(module, buffer.as_mut_ptr(), buffer.len() as u32) };
    if written == 0 {
        return None;
    }
    let path = String::from_utf16_lossy(&buffer[..written as usize]);
    std::path::Path::new(&path).parent().map(|p| p.to_path_buf())
}

/// Where the real system `version.dll` lives. A 32-bit process is redirected from
/// System32 to SysWOW64 automatically, so the plain System32 path is correct here.
pub fn system_version_dll() -> String {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    format!("{root}\\System32\\version.dll")
}
