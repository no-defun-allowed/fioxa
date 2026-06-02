#![no_std]

extern crate alloc;
#[macro_use]
extern crate userspace;
extern crate userspace_slaballoc;

use core::ffi::{c_char, c_int, CStr};
use core::ptr::null_mut;
use kernel_sys::{
    syscall::{sys_exit, sys_map, sys_unmap, sys_set_fs},
    types::VMMapFlags,
};

// Constants we need to fudge some syscalls with.
const ENOENT: c_int = 2;
const ENOTTY: c_int = 25;

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_log(message: *const c_char) {
    let c_str: &CStr = unsafe { CStr::from_ptr(message) };
    if let Ok(s) = c_str.to_str() {
        println!("{s}");
    } else {
        println!("chat why are we logging {message:?} when that isn't a UTF-8 string");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_panic() -> ! { panic!("libc panicked"); }

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_close(_fd: c_int) -> c_int {
    unimplemented!();
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_exit(status: c_int) -> ! {
    if status != 0 {
        println!("Exited with status {status}");
    }
    sys_exit();
}

// Memory

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_map(size: usize, result: *mut *mut ()) -> c_int {
    let addr = unsafe {
        sys_map(None, VMMapFlags::USERSPACE | VMMapFlags::WRITEABLE, null_mut(), size)
    };
    if let Ok(addr) = addr {
        unsafe { *result = addr };
        0
    } else {
        -1
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_unmap(addr: *mut (), size: usize) -> c_int {
    if unsafe { sys_unmap(addr, size) }.is_ok() { 0 } else { -1 }
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_set_fs_reg(addr: u64) { sys_set_fs(addr) }

// I/O

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_open(name: *const c_char, _flags: c_int, _mode: c_int, _fd: *mut c_int) -> c_int {
    fioxa_log(name);
    ENOENT
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_read(fd: c_int, _buf: *mut u8, count: usize, _bytes_read: *mut usize) -> c_int {
    unimplemented!("read {fd} {count}");
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_seek(fd: c_int, off: i64, whence: c_int, new_offset: *mut i64) -> c_int {
    unsafe { *new_offset = off };
    println!("pretending to seek {fd} {off} {whence}");
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_isatty(fd: c_int) -> c_int {
    if fd <= 2 { 0 } else { ENOTTY }
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_write(_fd: c_int, buf: *const u8, count: usize) {
    let buf = unsafe { core::slice::from_raw_parts(buf, count) };
    let _ = userspace::print::WRITER_STDOUT.lock().write_raw(buf);
}
