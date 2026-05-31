pub mod registers;

pub const AP_TRAMPOLINE: &[u8] = include_bytes!("ap_trampoline");

pub unsafe fn wrmsr(register: u32, value: u64) {
    unsafe {
        core::arch::asm!(
            "wrmsr",
            in("ecx") register,
            in("edx") (value >> 32) as u32,
            in("eax") value as u32,
            options(preserves_flags, nostack)
        );
    }
}

pub unsafe fn set_fs(value: u64) {
    unsafe { wrmsr(0xC0000100, value) }
}
