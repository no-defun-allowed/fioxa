#![no_std]

extern crate alloc;
#[macro_use]
extern crate userspace;
extern crate userspace_slaballoc;

mod fs;
mod gfx;

use alloc::{boxed::Box, ffi::CString, vec::Vec, vec};
use core::ffi::{c_char, c_int, c_long, CStr};
use core::ptr::null_mut;
use core::time::Duration;
use kernel_sys::{
    syscall::{sys_exit, sys_map, sys_set_fs, sys_sleep, sys_unmap, sys_uptime},
    types::VMMapFlags,
};

// Fudge the System V "stack" with auxv, envp, argc, argv
#[unsafe(no_mangle)]
pub extern "C" fn fioxa_fudge_sysv_stack() -> *const usize {
    let args = userspace::ARGS.read_vec();
    let args = str::from_utf8(&args).unwrap();
    let args = args.split(' '); // not really, but
    // Now make the stack.
    let mut v: Vec<usize> = vec![];
    v.push(0);                  // to fill in with argc later
    let mut argc = 1;
    v.push(c"fioxa".as_ptr() as usize);
    for arg in args {
        let s: *mut i8 = CString::new(arg).unwrap().into_raw();
        v.push(s as usize);
        argc += 1;
    }
    v[0] = argc;
    v.push(0);                  // argv terminator
    v.push(0);                  // envp terminator
    v.push(0);                  // null auxv
    let slice = v.into_boxed_slice();
    let leaked = Box::leak(slice);
    leaked.as_ptr()
}

// Internal mess-ups
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

// Now onto the "actual" syscalls.

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_clock(seconds: *mut c_long, nanoseconds: *mut c_long) -> c_int {
    let uptime = sys_uptime();
    unsafe {
        *seconds = (uptime / 1000) as c_long;
        *nanoseconds = ((uptime % 1000) * 1000000) as c_long;
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_exit(status: c_int) -> ! {
    if status != 0 {
        println!("Exited with status {status}");
    }
    sys_exit();
}

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
pub extern "C" fn fioxa_sleep(seconds: c_long, nanoseconds: c_long) -> c_int {
    let Ok(seconds): Result<u64, _> = seconds.try_into() else { return -1 };
    let Ok(nanoseconds): Result<u32, _> = nanoseconds.try_into() else { return -1 };
    sys_sleep(Duration::new(seconds, nanoseconds));
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_unmap(addr: *mut (), size: usize) -> c_int {
    if unsafe { sys_unmap(addr, size) }.is_ok() { 0 } else { -1 }
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_set_fs_reg(addr: u64) { sys_set_fs(addr) }

