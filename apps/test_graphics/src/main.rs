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

type FBClient = RPCClient<fb_capnp::FramebufferMessage>;

fn acquire(client: &mut FBClient) {
    let mut c = fb::Acquire::new_req();
    c.init();
    let r = client.send(&c.build()).unwrap();
    r.get_reply().unwrap();
}
fn release(client: &mut FBClient) {
    let mut c = fb::Release::new_req();
    c.init();
    let r = client.send(&c.build()).unwrap();
    r.get_reply().unwrap();
}

struct Framebuffer {
    capability: Handle,
    offset: usize,
    size: usize,
    width: u16,
    height: u16,
    stride: u16,
}

fn get_fb(client: &mut FBClient) -> Framebuffer {
    let mut c = fb::GetInfo::new_req();
    c.init();
    let mut r = client.send(&c.build()).unwrap();
    let mut h = r.take_handles_rpc();
    let mut r = r.get_reply().unwrap();
    let r = r.get_message().unwrap();
    println!("framebuffer at {:?} size {:?}, width {:?} height {:?} stride {:?}",
             r.get_offset(), r.get_size(),
             r.get_width(), r.get_height(), r.get_stride());
    Framebuffer {
        capability: h.take_handle(r.get_capability().unwrap()).unwrap(),
        offset: r.get_offset() as usize,
        size: r.get_size() as usize,
        width: r.get_width(),
        height: r.get_height(),
        stride: r.get_stride(),
    }
}

fn map_in_fb(fb: &Framebuffer) -> *mut () {
    unsafe {
        let base = sys_map(Some(*fb.capability), VMMapFlags::USERSPACE | VMMapFlags::WRITEABLE,
                           core::ptr::null_mut(),
                           fb.size).unwrap();
        base.add(fb.offset)
    }
}

fn draw_on(fb: &Framebuffer, mem: *mut ()) {
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
    let fb_channel = get_and_connect_service("FB").unwrap();
    let mut fb_client = FBClient::new(fb_channel);
    let fb = get_fb(&mut fb_client);
    acquire(&mut fb_client);
    let base = map_in_fb(&fb);
    println!("fb at {base:?}");
    draw_on(&fb, base);
    sys_sleep(Duration::from_millis(10000));
    release(&mut fb_client);
}
