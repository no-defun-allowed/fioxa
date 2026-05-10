use fioxa_rpc::client::RPCClient;
use kernel_userspace::handle::FIRST_HANDLE;

use crate::{bootfs::early_bootfs_get, elf, scheduling::process::ProcessReferences};

#[repr(C, packed)]
pub struct PartitionTableEntry {
    bootable: u8,
    _cfs: [u8; 3],
    partition_id: u8,
    _end_cfs: [u8; 3],
    start_lba: u32,
    length: u32,
}

#[repr(C, packed)]
pub struct MasterBootRecord {
    bootloader: [u8; 440],
    signature: u32,
    _unused: u16,

    partitions: [PartitionTableEntry; 4],
    magic_number: [u8; 2],
}

pub fn read_partitions(mut drive: RPCClient<fioxa_rpc::disk_capnp::DiskMessage>) {
    let mut req = fioxa_rpc::disk::Read::new_req();
    let mut b = req.init();
    b.set_sector(0);
    b.set_count(1);

    let mbr = drive.send(&req.build()).unwrap();
    let mut mbr = mbr.get_reply().unwrap();
    let mbr = mbr.get_message().unwrap().get_data().unwrap();

    let mbr = unsafe { &mut *(mbr.as_ptr() as *mut MasterBootRecord) };

    assert!(
        { mbr.magic_number } == [0x55, 0xAA],
        "MBR Magic number not valid, was given: {:?}",
        { mbr.magic_number }
    );

    for part in &mbr.partitions {
        if part.start_lba > 0 || part.bootable > 0 {
            info!(
                "Partition id {}: start:{} size:{}mb, bootable:{}",
                part.partition_id,
                { part.start_lba },
                part.length / 1024 * 512 / 1024,
                { part.bootable } == 0x80
            );

            let mut req = fioxa_rpc::disk::Restrict::new_req();
            let mut b = req.init();
            b.set_offset(part.start_lba as u64);
            b.set_length(part.length as u64);
            b.set_write(true);

            let mut r = drive.send(&req.build()).unwrap();
            let mut h = r.take_handles_rpc();
            let mut new = r.get_reply().unwrap();
            let new = new.get_message().unwrap().get_handle().unwrap();
            let fs_disk = h.take_handle(new).unwrap();

            elf::load_elf(early_bootfs_get("fat").unwrap())
                .unwrap()
                .references(ProcessReferences::from_refs(
                    [FIRST_HANDLE, *fs_disk].into_iter(),
                ))
                .privilege(crate::scheduling::process::ProcessPrivilege::USER)
                .build();
        }
    }
}
