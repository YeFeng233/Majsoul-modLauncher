//! PE export-table editing.
//!
//! Patching the Export Address Table - instead of overwriting machine code - needs no
//! instruction decoding, no trampoline and no relocation, which makes it safe on x86 and
//! immune to the prologue shape of the target function. Editing the table also means the
//! original code is never modified: calls that resolve the export afterwards land on our
//! function, while internal direct calls inside the DLL keep working untouched.

use core::ffi::c_void;

const EXPORT_DIRECTORY_INDEX: usize = 0;

unsafe fn rva_to_ptr(base: *mut u8, rva: u32) -> *mut u8 {
    base.add(rva as usize)
}

unsafe fn cstr_eq(ptr: *const u8, name: &str) -> bool {
    let bytes = name.as_bytes();
    for (i, expected) in bytes.iter().enumerate() {
        if *ptr.add(i) != *expected {
            return false;
        }
    }
    *ptr.add(bytes.len()) == 0
}

/// Address of the EAT slot that holds `name`'s function pointer.
unsafe fn export_slot(base: *mut u8, name: &str) -> Option<*mut u32> {
    if base.is_null() {
        return None;
    }
    let e_lfanew = *(base.add(0x3c) as *const u32) as usize;
    let nt = base.add(e_lfanew);
    if *(nt as *const u32) != 0x0000_4550 {
        return None;
    }
    let optional = nt.add(24);
    let magic = *(optional as *const u16);
    let data_dir = optional.add(if magic == 0x10b { 96 } else { 112 });
    let export_rva = *(data_dir.add(EXPORT_DIRECTORY_INDEX * 8) as *const u32);
    if export_rva == 0 {
        return None;
    }
    let export = rva_to_ptr(base, export_rva);
    let count = *(export.add(24) as *const u32) as usize;
    let functions = rva_to_ptr(base, *(export.add(28) as *const u32));
    let names = rva_to_ptr(base, *(export.add(32) as *const u32));
    let ordinals = rva_to_ptr(base, *(export.add(36) as *const u32));

    for i in 0..count {
        let name_rva = *(names.add(i * 4) as *const u32);
        if cstr_eq(rva_to_ptr(base, name_rva), name) {
            let ordinal = *(ordinals.add(i * 2) as *const u16) as usize;
            return Some(functions.add(ordinal * 4) as *mut u32);
        }
    }
    None
}

/// Point `name` at `replacement`, returning the previous target (the real function).
pub unsafe fn export_address(base: *mut u8, name: &str) -> Option<*mut c_void> {
    let rva = *export_slot(base, name)?;
    if rva == 0 { return None; }
    Some((base as usize).wrapping_add(rva as usize) as *mut c_void)
}

/// Point `name` at `replacement`, returning the previous target (the real function).
///
/// The export address table holds RVAs, and the loader adds the module base when the
/// export is resolved - so what gets stored has to be the *displacement* from `base`, even
/// though `replacement` normally lives in a different module. The 32-bit wraparound is
/// intentional: base + (replacement - base) yields exactly `replacement`.
pub unsafe fn replace_export(base: *mut u8, name: &str, replacement: *mut c_void) -> Option<*mut c_void> {
    let slot = export_slot(base, name)?;
    let previous_rva = *slot;
    if previous_rva == 0 {
        return None;
    }

    let mut old_protect = 0u32;
    if crate::win::VirtualProtect(
        slot as *mut c_void,
        core::mem::size_of::<u32>(),
        crate::win::PAGE_READWRITE,
        &mut old_protect,
    ) == 0
    {
        return None;
    }
    *slot = (replacement as usize).wrapping_sub(base as usize) as u32;
    crate::win::VirtualProtect(
        slot as *mut c_void,
        core::mem::size_of::<u32>(),
        old_protect,
        &mut old_protect,
    );

    Some((base as usize).wrapping_add(previous_rva as usize) as *mut c_void)
}
