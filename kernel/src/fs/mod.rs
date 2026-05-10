pub mod mbr;

use fioxa_rpc::{
    client::RPCClient,
    service::{connect_service, get_services},
};

use crate::fs::mbr::read_partitions;

pub fn file_system_partition_loader() {
    get_services("DISK", true, |disk| {
        read_partitions(RPCClient::new(connect_service(&disk).unwrap()));
    })
    .unwrap();

    panic!("the iterator should never end")
}
