#![no_std]

extern crate alloc;
#[macro_use]
extern crate userspace;
extern crate userspace_slaballoc;

use core::ffi::{c_char, c_int, CStr};
use kernel_sys::{syscall::{sys_exit, sys_set_fs}};

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

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_open(_name: *const c_char, _flags: c_int, _mode: c_int, _fd: *mut c_int) -> c_int {
    unimplemented!();
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_set_fs_reg(addr: u64) { sys_set_fs(addr) }

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_read(_fd: c_int, _buf: *mut u8, _count: usize, _bytes_read: *mut usize) -> c_int {
    unreachable!();
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_write(_fd: c_int, buf: *const u8, count: usize) {
    let buf = unsafe { core::slice::from_raw_parts(buf, count) };
    let _ = userspace::print::WRITER_STDOUT.lock().write_raw(buf);
}
