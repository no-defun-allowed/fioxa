#![no_std]
#![no_main]

extern crate alloc;
#[macro_use]
extern crate userspace;
extern crate userspace_slaballoc;

init_userspace!(main);

use kernel_sys::{syscall::{sys_set_fs, sys_sleep, sys_yield}};
use core::time::Duration;

pub fn main() {
    let magic_number = 0x3140;
    sys_set_fs(magic_number);
    sys_yield();
    println!("I think I shall cause a fault at {magic_number:x} on purpose");
    sys_sleep(Duration::from_secs(1));
    let _x: u64;
    unsafe { core::arch::asm!("mov {}, fs:0", out(reg) _x); }
}
