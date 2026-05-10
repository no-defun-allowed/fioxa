use core::{mem::size_of, pin::Pin, time::Duration};

use alloc::boxed::Box;
use fioxa_rpc::disk_capnp;
use kernel_sys::syscall::sys_sleep;
use kernel_userspace::disk::ata::ATADiskIdentify;

use crate::{
    cpu_localstorage::CPULocalStorageRW,
    driver::disk::ahci::{
        HBACommandTable,
        fis::{FISTYPE, FisRegH2D},
    },
};

use super::{
    HBA_PX_CMD_CR, HBA_PX_CMD_FR, HBA_PX_CMD_FREE, HBA_PX_CMD_ST, HBAPort,
    bitfields::HBACommandHeader, fis::ReceivedFis,
};

#[derive(Debug, PartialEq)]
pub enum PortType {
    None = 0,
    SATA = 1,
    SEMB = 2,
    PM = 3,
    SATAPI = 4,
}

pub const PRDT_LENGTH: usize = 8;

// because of alignment we can't ensure a full transfer
const MAX_SECTORS: usize = (PRDT_LENGTH - 1) * 8;

#[allow(dead_code)]
pub struct Port {
    hba_port: &'static mut HBAPort,
    received_fis: Pin<Box<ReceivedFis>>,
    cmd_list: Pin<Box<[HBACommandHeader; 32]>>,
    cmd_tables: Pin<Box<[HBACommandTable<PRDT_LENGTH>; 32]>>,
}

unsafe fn get_phys_addr_from_vaddr(address: u64) -> Option<u64> {
    unsafe {
        let thread = CPULocalStorageRW::get_current_task();
        let mem = thread.process().memory.lock();
        mem.region.get_phys_addr_from_vaddr(address)
    }
}

unsafe fn setup_prdts(
    cmd_table: &mut HBACommandTable<PRDT_LENGTH>,
    vstart: *const (),
    vlen: u32,
) -> u16 {
    let mut ptr_addr = vstart as u64;
    let left_align_size = (ptr_addr & 0xFFF) as u32;
    let mut bytes_to_read = vlen;
    let mut prdt_length = 0u16;
    if left_align_size > 0 {
        // Align ptr on prev boundary
        ptr_addr &= !0xFFF;

        let phys_addr = unsafe { get_phys_addr_from_vaddr(ptr_addr).unwrap() };

        // Set the offset back on, since page offsets arn't supper pain yet (Only 4kb pages)
        cmd_table.prdt_entry[0].set_data_base_address(phys_addr + left_align_size as u64);

        cmd_table.prdt_entry[0].set_byte_count(0xFFF - left_align_size);
        // cmd_table.prdt_entry[0].set_interrupt_on_completion(true);
        prdt_length = 1;
        // Might have requested less than 0x1000 bytes
        bytes_to_read = bytes_to_read.saturating_sub(0x1000 - left_align_size);
        ptr_addr += 0x1000;
    }

    while bytes_to_read > 0x1000 {
        let phys_addr = unsafe { get_phys_addr_from_vaddr(ptr_addr).unwrap() };
        cmd_table.prdt_entry[prdt_length as usize].set_data_base_address(phys_addr);
        // Read read of bytes
        cmd_table.prdt_entry[prdt_length as usize].set_byte_count(0xFFF);
        bytes_to_read -= 0x1000;
        ptr_addr += 0x1000;
        prdt_length += 1;
    }

    if bytes_to_read > 0 {
        let phys_addr = unsafe { get_phys_addr_from_vaddr(ptr_addr).unwrap() };

        cmd_table.prdt_entry[prdt_length as usize].set_data_base_address(phys_addr);
        // Read read of bytes
        cmd_table.prdt_entry[prdt_length as usize].set_byte_count(bytes_to_read - 1);
        prdt_length += 1;
    }

    prdt_length
}

fn set_cmd_fis_lba(cmd_fis: &mut FisRegH2D, sector: u64) {
    let sector_low = sector as u32;
    let sector_high = (sector >> 32) as u32;

    cmd_fis.set_lba0(sector_low as u8);
    cmd_fis.set_lba1((sector_low >> 8) as u8);
    cmd_fis.set_lba2((sector_low >> 16) as u8);

    cmd_fis.set_lba3(sector_high as u8);
    cmd_fis.set_lba4((sector_high >> 8) as u8);
    cmd_fis.set_lba5((sector_high >> 16) as u8);
}

impl Port {
    pub fn new(port: &'static mut HBAPort) -> Self {
        unsafe {
            Self::stop_cmd(port);

            let received_fis: Pin<Box<ReceivedFis>> =
                Box::into_pin(Box::new_uninit().assume_init());
            let rfis_addr =
                get_phys_addr_from_vaddr(&*received_fis.as_ref() as *const ReceivedFis as u64)
                    .unwrap();

            let mut cmd_list: Pin<Box<[HBACommandHeader; 32]>> =
                Box::into_pin(Box::new_uninit().assume_init());
            let cmd_list_addr = get_phys_addr_from_vaddr(cmd_list.as_ptr() as u64).unwrap();

            port.command_list_base.write(cmd_list_addr);
            port.fis_base_address.write(rfis_addr);

            let cmd_tables: Pin<Box<[HBACommandTable<PRDT_LENGTH>; 32]>> =
                Box::into_pin(Box::new_uninit().assume_init());
            let cmd_tables_addr: u64 =
                get_phys_addr_from_vaddr(cmd_tables.as_ptr() as u64).unwrap();

            for i in 0..32 {
                cmd_list[i].set_prdt_length(0);
                cmd_list[i].set_command_table_base_address(
                    cmd_tables_addr + i as u64 * size_of::<HBACommandTable<PRDT_LENGTH>>() as u64,
                );
            }

            // port.sata_active.write(u32::MAX);

            Self::start_cmd(port);
            Self {
                hba_port: port,
                received_fis,
                cmd_list,
                cmd_tables,
            }
        }
    }

    pub fn find_slot(&mut self) -> u8 {
        let test = self.hba_port.command_issue.read() | self.hba_port.sata_active.read();
        loop {
            for slot in 0..32 {
                if test & (1 << slot) == 0 {
                    return slot;
                }
            }
            sys_sleep(Duration::from_millis(10));
        }
    }

    pub fn start_cmd(port: &mut HBAPort) {
        while port.cmd_sts.read() & HBA_PX_CMD_CR > 0 {
            // yield_now();
        }

        port.cmd_sts.update(|v| *v |= HBA_PX_CMD_FREE);
        port.cmd_sts.update(|v| *v |= HBA_PX_CMD_ST);
    }

    pub fn stop_cmd(port: &mut HBAPort) {
        // Stop port
        port.cmd_sts.update(|x| *x &= !HBA_PX_CMD_ST);
        // LIST_ON
        while port.cmd_sts.read() & HBA_PX_CMD_CR > 0 {}

        port.cmd_sts.update(|x| *x &= !HBA_PX_CMD_FREE);
        while port.cmd_sts.read() & HBA_PX_CMD_FR > 0 {}
    }

    fn issue(&mut self, slot: u8) -> Option<()> {
        let mut spin = 100_000;

        while ((self.hba_port.task_file_data.read() & (0x80 | 0x08)) > 0) && spin > 0 {
            spin -= 1;
            // yield_now();
        }
        if spin == 0 {
            error!("Port is hung");
            return None;
        }

        self.hba_port.command_issue.write(1 << slot);
        loop {
            // yield_now();
            if self.hba_port.command_issue.read() & (1 << slot) == 0 {
                break;
            }
            if self.hba_port.interrupt_status.read() & (1 << 30) > 0 {
                debug!("Err");
                return None;
                // Read error
            }
        }
        if self.hba_port.interrupt_status.read() & (1 << 30) > 0 {
            debug!("Err");
            return None; // Read error
        }

        Some(())
    }

    fn read_into_buf(&mut self, sector: u64, sector_count: u32, buffer: &mut [u8]) -> Option<()> {
        if sector_count as usize > MAX_SECTORS {
            todo!("Sectors count of {MAX_SECTORS} is max atm")
        }

        assert!(
            buffer.len() >= sector_count as usize * 512,
            "Buffer is not large enough"
        );

        let slot = self.find_slot();

        let cmd_list = &mut self.cmd_list[slot as usize];
        cmd_list.set_command_fis_length((size_of::<FisRegH2D>() / 4) as u8);
        cmd_list.set_write(false); // This is read

        let cmd_table = &mut self.cmd_tables[slot as usize];

        let prdt_length =
            unsafe { setup_prdts(cmd_table, buffer.as_ptr().cast(), sector_count * 512) };

        cmd_list.set_prdt_length(prdt_length);

        let cmd_fis = unsafe { &mut *(cmd_table.command_fis.as_mut_ptr() as *mut FisRegH2D) };
        cmd_fis.set_fis_type(FISTYPE::REGH2D as u8);
        cmd_fis.set_control(1); // COMMAND

        const ATA_CMD_READ_DMA_EX: u8 = 0x25;
        cmd_fis.set_command(ATA_CMD_READ_DMA_EX);
        cmd_fis.set_command_control(true);

        set_cmd_fis_lba(cmd_fis, sector);
        cmd_fis.set_device_register(1 << 6); // LBA mode

        cmd_fis.set_countl((sector_count & 0xFF) as u8);
        cmd_fis.set_counth(((sector_count >> 8) & 0xFF) as u8);

        self.issue(slot)
    }

    fn write_from_buf(&mut self, sector: u64, sector_count: u32, buffer: &[u8]) -> Option<()> {
        if sector_count as usize > MAX_SECTORS {
            todo!("Sectors count of {MAX_SECTORS} is max atm")
        }

        assert!(
            buffer.len() >= sector_count as usize * 512,
            "Buffer is not large enough"
        );

        let slot = self.find_slot();

        let cmd_list = &mut self.cmd_list[slot as usize];
        cmd_list.set_command_fis_length((size_of::<FisRegH2D>() / 4) as u8);
        cmd_list.set_write(true); // This is write

        let cmd_table = &mut self.cmd_tables[slot as usize];

        let prdt_length =
            unsafe { setup_prdts(cmd_table, buffer.as_ptr().cast(), sector_count * 512) };

        cmd_list.set_prdt_length(prdt_length);

        let cmd_fis = unsafe { &mut *(cmd_table.command_fis.as_mut_ptr() as *mut FisRegH2D) };
        cmd_fis.set_fis_type(FISTYPE::REGH2D as u8);
        cmd_fis.set_control(1); // COMMAND

        const ATA_CMD_WRITE_DMA_EX: u8 = 0x35;
        cmd_fis.set_command(ATA_CMD_WRITE_DMA_EX);
        cmd_fis.set_command_control(true);

        set_cmd_fis_lba(cmd_fis, sector);
        cmd_fis.set_device_register(1 << 6); // LBA mode

        cmd_fis.set_countl((sector_count & 0xFF) as u8);
        cmd_fis.set_counth(((sector_count >> 8) & 0xFF) as u8);

        self.issue(slot)
    }

    pub fn read(&mut self, sector: u64, count: u32, buffer: &mut [u8]) {
        let mut read_head = 0usize;
        let sector = sector as usize;
        let read_tail = count as usize;

        while read_head < read_tail {
            let count = (read_tail - read_head).min(MAX_SECTORS);
            self.read_into_buf(
                (sector + read_head) as u64,
                count as u32,
                &mut buffer[read_head * 512..(read_head + count) * 512],
            )
            .unwrap();
            read_head += count;
        }
    }

    pub fn write(&mut self, sector: u64, data: &[u8]) {
        let mut write_head = 0usize;
        let sector = sector as usize;
        let write_tail = data.len() / 512;

        while write_head < write_tail {
            let count = (write_tail - write_head).min(MAX_SECTORS);
            self.write_from_buf(
                (sector + write_head) as u64,
                count as u32,
                &data[write_head * 512..(write_head + count) * 512],
            )
            .unwrap();
            write_head += count;
        }
    }
}

impl fioxa_rpc::disk::Service for Port {
    fn read<'a>(
        &mut self,
        req: fioxa_rpc::OwnedReader<'a, disk_capnp::read::Owned>,
        _req_handles: ::alloc::vec::Vec<::kernel_userspace::handle::Handle>,
        res: fioxa_rpc::OwnedBuilder<'a, disk_capnp::read_resp::Owned>,
        _res_handles: &'a mut fioxa_rpc::RPCHandleBuilder<'static>,
    ) -> Result<(), ::capnp::Error> {
        let length = req
            .get_count()
            .checked_mul(512)
            .ok_or_else(|| capnp::Error::failed("length overflow".into()))?;

        let buffer = res.init_data(length);

        let mut read_head = 0usize;
        let sector = req.get_sector() as usize;
        let read_tail = req.get_count() as usize;

        while read_head < read_tail {
            let count = (read_tail - read_head).min(MAX_SECTORS);
            self.read_into_buf(
                (sector + read_head) as u64,
                count as u32,
                &mut buffer[read_head * 512..(read_head + count) * 512],
            )
            .unwrap();
            read_head += count;
        }

        Ok(())
    }

    fn identify<'a>(
        &mut self,
        _req: fioxa_rpc::OwnedReader<'a, disk_capnp::identify::Owned>,
        _req_handles: ::alloc::vec::Vec<::kernel_userspace::handle::Handle>,
        res: fioxa_rpc::OwnedBuilder<'a, disk_capnp::read_resp::Owned>,
        _res_handles: &'a mut fioxa_rpc::RPCHandleBuilder<'static>,
    ) -> Result<(), ::capnp::Error> {
        let buffer = res.init_data(512);

        self.hba_port.interrupt_status.write(0xFFFFFFFF);
        let slot = self.find_slot() as usize;

        let cmd_list = &mut self.cmd_list[slot];
        cmd_list.set_command_fis_length((size_of::<FisRegH2D>() / 4) as u8);
        cmd_list.set_write(false); // This is read

        let cmd_table = &mut self.cmd_tables[slot];

        let start_addr = buffer.as_ptr() as usize;

        let page_1_free = 0x1000 - (start_addr & 0xFFF);
        let page_1_addr = unsafe { get_phys_addr_from_vaddr(start_addr as u64).unwrap() };

        cmd_table.prdt_entry[0].set_data_base_address(page_1_addr);
        cmd_table.prdt_entry[0]
            .set_byte_count(size_of::<ATADiskIdentify>().min(page_1_free) as u32 - 1);

        if page_1_free < size_of::<ATADiskIdentify>() {
            let page_2_addr =
                unsafe { get_phys_addr_from_vaddr(start_addr as u64 + 0x1000).unwrap() };
            cmd_table.prdt_entry[1].set_data_base_address(page_2_addr);
            cmd_table.prdt_entry[1]
                .set_byte_count((size_of::<ATADiskIdentify>() - page_1_free) as u32 - 1);
            cmd_list.set_prdt_length(2);
        } else {
            cmd_list.set_prdt_length(1);
        }

        let cmd_fis = unsafe { &mut *(cmd_table.command_fis.as_mut_ptr() as *mut FisRegH2D) };
        cmd_fis.set_fis_type(FISTYPE::REGH2D as u8);
        // cmd_fis.set_control(1);

        cmd_fis.set_command(0xec); // Ident
        cmd_fis.set_countl(0);
        cmd_fis.set_command_control(true);

        self.issue(slot as u8).unwrap();
        Ok(())
    }

    fn write<'a>(
        &mut self,
        req: fioxa_rpc::OwnedReader<'a, disk_capnp::write::Owned>,
        _req_handles: ::alloc::vec::Vec<::kernel_userspace::handle::Handle>,
        _res: fioxa_rpc::OwnedBuilder<'a, disk_capnp::write_resp::Owned>,
        _res_handles: &'a mut fioxa_rpc::RPCHandleBuilder<'static>,
    ) -> Result<(), ::capnp::Error> {
        let data = req.get_data()?;

        if data.is_empty() {
            return Ok(());
        }

        if !data.len().is_multiple_of(512) {
            return Err(capnp::Error::failed(
                "data must be a multiple of sector size (512)".into(),
            ));
        }

        let mut write_head = 0usize;
        let sector = req.get_sector() as usize;
        let write_tail = data.len() / 512;

        while write_head < write_tail {
            let count = (write_tail - write_head).min(MAX_SECTORS);
            self.write_from_buf(
                (sector + write_head) as u64,
                count as u32,
                &data[write_head * 512..(write_head + count) * 512],
            )
            .unwrap();
            write_head += count;
        }

        Ok(())
    }
}
