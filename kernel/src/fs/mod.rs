pub mod fat;
pub mod mbr;

use alloc::sync::Arc;
use fioxa_rpc::{
    client::{RPCClient, RPCMessageReturn},
    service::{connect_service, get_services},
};
use kernel_userspace::mutex::Mutex;

use crate::fs::mbr::read_partitions;

pub struct FSPartitionDisk {
    backing_disk: Arc<Mutex<RPCClient<fioxa_rpc::disk_capnp::DiskMessage>>>,
    partition_offset: u64,
    partition_length: u64,
}

impl FSPartitionDisk {
    pub fn new(
        backing_disk: Arc<Mutex<RPCClient<fioxa_rpc::disk_capnp::DiskMessage>>>,
        partition_offset: u64,
        partition_length: u64,
    ) -> Self {
        Self {
            backing_disk,
            partition_offset,
            partition_length,
        }
    }

    fn read(
        &self,
        sector: u64,
        sector_count: u32,
    ) -> RPCMessageReturn<fioxa_rpc::disk_capnp::read_resp::Owned> {
        assert!(sector + sector_count as u64 <= self.partition_length);
        let mut req = fioxa_rpc::disk::Read::new_req();
        let mut b = req.init();
        b.set_sector(self.partition_offset + sector);
        b.set_count(sector_count);

        let mut d = self.backing_disk.lock();
        d.send(&req.build()).unwrap()
    }

    fn narrow(&self, start: u64, length: u64) -> Self {
        assert!(start + length <= self.partition_length);

        Self {
            backing_disk: self.backing_disk.clone(),
            partition_offset: self.partition_offset + start,
            partition_length: length,
        }
    }
}

pub fn file_system_partition_loader() {
    get_services("DISK", true, |disk| {
        read_partitions(FSPartitionDisk::new(
            Arc::new(Mutex::new(RPCClient::new(connect_service(&disk).unwrap()))),
            0,
            u64::MAX,
        ));
    })
    .unwrap();

    panic!("the iterator should never end")
}
