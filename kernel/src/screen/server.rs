use crate::paging::virt_addr_offset;
use crate::BOOT_INFO;
use fioxa_rpc::{fb, fb_capnp, server::RPCServer, service::ServiceExecutor};
use kernel_sys::{
    syscall::{sys_process_spawn_thread, sys_vmo_mmap_create},
};
use kernel_userspace::handle::Handle;

struct Framebuffer();

pub fn fb_server_task() {
    ServiceExecutor::with_name("FB", |chan| {
        sys_process_spawn_thread(move || {
            RPCServer::new(chan, Framebuffer()).run()
        });
    }).run().unwrap();
}

impl fb::FramebufferService for Framebuffer {
    fn get_info<'a>(
        &mut self,
        _req: fioxa_rpc::OwnedReader<'a, fb_capnp::framebuffer_get_info::Owned>,
        _req_handles: ::alloc::vec::Vec<::kernel_userspace::handle::Handle>,
        mut res: fioxa_rpc::OwnedBuilder<'a, fb_capnp::framebuffer_info::Owned>,
        res_handles: &'a mut fioxa_rpc::RPCHandleBuilder<'static>,
    ) -> Result<(), ::capnp::Error> {
        let gop = unsafe { &(*virt_addr_offset(BOOT_INFO)).gop };
        let fb_ptr = unsafe { *gop.buffer.as_ptr() as usize };
        let fb_base = fb_ptr & !0xFFF;
        let fb_top = (fb_base + gop.buffer_size + 0xFFF) & !0xFFF;
        let hid = unsafe { sys_vmo_mmap_create(fb_base as *mut (), fb_top - fb_base) };
        let handle = unsafe { Handle::from_id(hid) };
        res.set_offset((fb_ptr - fb_base) as u64);
        res.set_size((fb_top - fb_base) as u64);
        res.set_width(gop.horizontal as u16);
        res.set_height(gop.vertical as u16);
        res.set_stride(gop.stride as u16);
        res_handles.add(res.init_capability(), handle);
        Ok(())
    }
    fn acquire<'a>(
        &mut self,
        _req: fioxa_rpc::OwnedReader<'a, fb_capnp::framebuffer_acquire::Owned>,
        _req_handles: ::alloc::vec::Vec<::kernel_userspace::handle::Handle>,
        _res: fioxa_rpc::OwnedBuilder<'a, fb_capnp::framebuffer_changed::Owned>,
        _res_handles: &'a mut fioxa_rpc::RPCHandleBuilder<'static>,
    ) -> Result<(), ::capnp::Error> {
        super::set_kernel_drawing_screen(false);
        Ok(())
    }
    fn release<'a>(
        &mut self,
        _req: fioxa_rpc::OwnedReader<'a, fb_capnp::framebuffer_release::Owned>,
        _req_handles: ::alloc::vec::Vec<::kernel_userspace::handle::Handle>,
        _res: fioxa_rpc::OwnedBuilder<'a, fb_capnp::framebuffer_changed::Owned>,
        _res_handles: &'a mut fioxa_rpc::RPCHandleBuilder<'static>,
    ) -> Result<(), ::capnp::Error> {
        let writer = super::gop::WRITER.get().unwrap();
        writer.lock().dirty_everything();
        super::set_kernel_drawing_screen(true);
        Ok(())
    }
}
