#![no_std]
#![no_main]

use fioxa_rpc::{client::RPCClient, service::get_and_connect_service};
use kernel_userspace::net::IPAddr;
use userspace::ARGS;

extern crate alloc;
#[macro_use]
extern crate userspace;
extern crate userspace_slaballoc;

init_userspace!(main);

pub fn main() {
    let args = ARGS.read_vec();
    let args = str::from_utf8(&args).unwrap();

    let mut args = args.split_whitespace();

    let cmd = args.next().expect("please provide args");

    match cmd.to_uppercase().as_str() {
        "ARP" => {
            let mut ip = args.next().unwrap().split('.');
            let a = ip.next().unwrap();
            let b = ip.next().unwrap();
            let c = ip.next().unwrap();
            let d = ip.next().unwrap();
            let ip = IPAddr::V4(
                a.parse().unwrap(),
                b.parse().unwrap(),
                c.parse().unwrap(),
                d.parse().unwrap(),
            );

            let networking = get_and_connect_service("NETWORKING").unwrap();
            let mut networking = RPCClient::new(networking);

            let mut req = fioxa_rpc::net::ArpRequest::new_req();
            req.init().set_ip(ip.as_net_be());
            let r = networking.send(&req.build()).unwrap();
            let mut r = r.get_reply().unwrap();
            let r = r.get_message().unwrap();
            match r.get_success().unwrap() {
                fioxa_rpc::net_capnp::arp_reponse::ArpSuccess::Success => {
                    println!("{a}.{b}.{c}.{d} = {:#X?}", r.get_mac());
                }
                fioxa_rpc::net_capnp::arp_reponse::ArpSuccess::NotSameSubnet => {
                    println!("error: NotSameSubnet");
                }
                fioxa_rpc::net_capnp::arp_reponse::ArpSuccess::Unknown => {
                    println!("pending answer, try again later");
                }
            }
        }
        _ => println!("Unknown cmd"),
    }
}
