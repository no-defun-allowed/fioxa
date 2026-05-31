#![no_std]
#![no_main]

extern crate alloc;
#[macro_use]
extern crate userspace;
extern crate userspace_slaballoc;

init_userspace!(main);

use kernel_sys::{syscall::{sys_set_fs, sys_yield}};

fn read_fs() -> u64 {
    let result;
    unsafe { core::arch::asm!("mov {}, fs", out(reg) result) };
    result
}

pub fn main() {
    sys_set_fs(42);
    println!("before yield: fs = {}", read_fs());
    sys_yield();
    println!(" after yield: fs = {}", read_fs());
}
