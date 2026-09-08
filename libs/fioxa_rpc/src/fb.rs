use crate::fb_capnp;

crate::generate_rpc!(crate::fb_capnp::FramebufferMessage, FramebufferService;
    GetInfo @ GetInfo @ get_info(fb_capnp::framebuffer_get_info::Owned) -> fb_capnp::framebuffer_info::Owned;
    Acquire @ Acquire @ acquire(fb_capnp::framebuffer_acquire::Owned) -> fb_capnp::framebuffer_changed::Owned;
    Release @ Release @ release(fb_capnp::framebuffer_release::Owned) -> fb_capnp::framebuffer_changed::Owned;
);


use crate::{
    client::RPCClient,
    service::get_and_connect_service,
};
use kernel_sys::syscall::sys_map;
use kernel_sys::types::VMMapFlags;
use kernel_userspace::handle::Handle;

pub type FBClient = RPCClient<fb_capnp::FramebufferMessage>;

impl FBClient {
    pub fn connect() -> Self {
        let channel = get_and_connect_service("FB").unwrap();
        FBClient::new(channel)
    }
    pub fn acquire(&mut self) {
        let mut c = Acquire::new_req();
        c.init();
        let r = self.send(&c.build()).unwrap();
        r.get_reply().unwrap();
    }
    
    pub fn release(&mut self) {
        let mut c = Release::new_req();
        c.init();
        let r = self.send(&c.build()).unwrap();
        r.get_reply().unwrap();
    }

    pub fn get_framebuffer(&mut self) -> Framebuffer {
        let mut c = GetInfo::new_req();
        c.init();
        let mut r = self.send(&c.build()).unwrap();
        let mut h = r.take_handles_rpc();
        let mut r = r.get_reply().unwrap();
        let r = r.get_message().unwrap();
        Framebuffer {
            capability: h.take_handle(r.get_capability().unwrap()).unwrap(),
            offset: r.get_offset() as usize,
            size: r.get_size() as usize,
            width: r.get_width(),
            height: r.get_height(),
            stride: r.get_stride(),
        }
    }
}

#[derive(Debug)]
pub struct Framebuffer {
    pub capability: Handle,
    pub offset: usize,
    pub size: usize,
    pub width: u16,
    pub height: u16,
    pub stride: u16,
}

impl Framebuffer {
    pub fn map(&self) -> *mut () {
        unsafe {
            let base = sys_map(Some(*self.capability), VMMapFlags::USERSPACE | VMMapFlags::WRITEABLE,
                               core::ptr::null_mut(),
                               self.size).unwrap();
            base.add(self.offset)
        }
    }
}
