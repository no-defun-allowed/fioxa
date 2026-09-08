#![no_std]
#![no_main]

use core::time::Duration;
use fioxa_rpc::{
    client::RPCClient,
    fb, fb_capnp,
    service::get_and_connect_service,
};
use kernel_sys::syscall::{sys_handle_drop, sys_map, sys_vmo_mmap_create, sys_sleep};
use kernel_sys::types::VMMapFlags;
use kernel_userspace::handle::Handle;

extern crate alloc;
#[macro_use]
extern crate userspace;
extern crate userspace_slaballoc;

mod mandelbrot;

init_userspace!(main);

fn draw_on(fb: &fb::Framebuffer, mem: *mut ()) {
    let mem = mem as *mut u8;
    for y in 0..fb.height {
        for x in 0..fb.width {
            let loc = 4 * (fb.stride as usize * y as usize + x as usize);
            let c = mandelbrot::mandel(fb.width, fb.height, 100, -2.0, 0.5, -1.0, 1.0, x, y);
            let color = u32::from_le_bytes([c.0, c.1, c.2, 0xFF]);
            unsafe { core::ptr::write_volatile(mem.add(loc) as *mut u32, color) }
        }
    }
}

pub fn main() {
    let mut fb_client = fb::FBClient::connect();
    let fb = fb_client.get_framebuffer();
    println!("Got {fb:?}");
    fb_client.acquire();
    let base = fb.map();
    draw_on(&fb, base);
    sys_sleep(Duration::from_millis(10000));
    fb_client.release();
}
