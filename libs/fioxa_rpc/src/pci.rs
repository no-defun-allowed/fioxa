use crate::{
    client::RPCClient,
    pci_capnp::{self, PCIMessage},
};
use alloc::sync::Arc;
use kernel_userspace::mutex::Mutex;

crate::generate_rpc!(pci_capnp::PCIMessage, Service;
    Read @ Read @ read(pci_capnp::read::Owned) -> pci_capnp::read_res::Owned;
    Write @ Write @ write(pci_capnp::write::Owned) -> pci_capnp::write_res::Owned;
);

pub struct PCIDevice(pub RPCClient<PCIMessage>);

#[allow(dead_code)]
impl PCIDevice {
    pub const fn new(client: RPCClient<PCIMessage>) -> Self {
        Self(client)
    }

    pub fn into_inner(self) -> RPCClient<PCIMessage> {
        self.0
    }

    pub const fn client(&mut self) -> &mut RPCClient<PCIMessage> {
        &mut self.0
    }

    unsafe fn read(&mut self, offset: u32, size: pci_capnp::Size) -> u32 {
        let mut c = Read::new_req();
        let mut b = c.init();
        b.set_offset(offset);
        b.set_size(size);
        let r = self.client().send(&c.build()).unwrap();
        let mut r = r.get_reply().unwrap();
        r.get_message().unwrap().get_val()
    }

    unsafe fn read_u8(&mut self, offset: u32) -> u8 {
        unsafe { self.read(offset, pci_capnp::Size::U8) as u8 }
    }

    unsafe fn read_u16(&mut self, offset: u32) -> u16 {
        unsafe { self.read(offset, pci_capnp::Size::U16) as u16 }
    }

    unsafe fn read_u32(&mut self, offset: u32) -> u32 {
        unsafe { self.read(offset, pci_capnp::Size::U32) }
    }

    unsafe fn write(&mut self, offset: u32, size: pci_capnp::Size, val: u32) {
        let mut c = Write::new_req();
        let mut b = c.init();
        b.set_offset(offset);
        b.set_size(size);
        b.set_val(val);
        let r = self.client().send(&c.build()).unwrap();
        let mut r = r.get_reply().unwrap();
        r.get_message().unwrap();
    }

    unsafe fn write_u8(&mut self, offset: u32, data: u8) {
        unsafe { self.write(offset, pci_capnp::Size::U8, data as u32) }
    }

    unsafe fn write_u16(&mut self, offset: u32, data: u16) {
        unsafe { self.write(offset, pci_capnp::Size::U16, data as u32) }
    }

    unsafe fn write_u32(&mut self, offset: u32, data: u32) {
        unsafe { self.write(offset, pci_capnp::Size::U32, data) }
    }
}

pub struct PCIHeaderCommon {
    pub device: Arc<Mutex<PCIDevice>>,
}

impl PCIHeaderCommon {
    pub fn get_vendor_id(&self) -> u16 {
        unsafe { self.device.lock().read_u16(0) }
    }
    pub fn get_device_id(&self) -> u16 {
        unsafe { self.device.lock().read_u16(2) }
    }

    pub fn get_command(&self) -> u16 {
        unsafe { self.device.lock().read_u16(4) }
    }

    pub fn get_status(&self) -> u16 {
        unsafe { self.device.lock().read_u16(6) }
    }

    pub fn get_revision_id(&self) -> u8 {
        unsafe { self.device.lock().read_u8(8) }
    }

    pub fn get_prog_if(&self) -> u8 {
        unsafe { self.device.lock().read_u8(9) }
    }

    pub fn set_prog_if(&self) -> u8 {
        unsafe { self.device.lock().read_u8(9) }
    }

    pub fn get_subclass(&self) -> u8 {
        unsafe { self.device.lock().read_u8(10) }
    }

    pub fn get_class(&self) -> u8 {
        unsafe { self.device.lock().read_u8(11) }
    }

    pub fn get_cache_line_size(&self) -> u8 {
        unsafe { self.device.lock().read_u8(12) }
    }

    pub fn get_latency_timer(&self) -> u8 {
        unsafe { self.device.lock().read_u8(13) }
    }

    pub fn get_header_type(&self) -> u8 {
        unsafe { self.device.lock().read_u8(14) }
    }

    pub fn get_bist(&self) -> u8 {
        unsafe { self.device.lock().read_u8(15) }
    }

    /// # Safety
    ///
    /// The caller must ensure the device is of the correct type
    pub unsafe fn get_as_header0(self) -> PCIHeader0 {
        PCIHeader0 {
            device: self.device.clone(),
        }
    }
}

pub struct PCIHeader0 {
    device: Arc<Mutex<PCIDevice>>,
}

impl PCIHeader0 {
    pub fn common(&self) -> PCIHeaderCommon {
        PCIHeaderCommon {
            device: self.device.clone(),
        }
    }

    pub fn get_port_base(&self) -> Option<u32> {
        for i in 0..5 {
            let bar = self.get_bar(i);
            let address = bar & 0xFFFFFFFC;
            if address > 0 && bar & 1 == 1 {
                return Some(address);
            }
        }
        None
    }

    pub fn get_bar(&self, bar_num: u8) -> u32 {
        assert!(bar_num <= 5);
        unsafe { self.device.lock().read_u32(0x10 + bar_num as u32 * 4) }
    }

    pub fn get_interrupt_num(&self) -> u8 {
        unsafe { self.device.lock().read_u8(0x3C) }
    }
}
