use kernel_userspace::interrupt::Interrupt;

use crate::{
    client::RPCClient,
    interrupt_capnp::{InterruptMessage, Vector},
    service::get_and_connect_service,
};

crate::generate_rpc!(crate::interrupt_capnp::InterruptMessage, Service;
    Subscribe @ Subscribe @ subscribe(crate::interrupt_capnp::subscribe::Owned) -> crate::interrupt_capnp::subscribe_resp::Owned;
);

pub struct InterruptClient(RPCClient<InterruptMessage>);

impl InterruptClient {
    pub const fn new(client: RPCClient<InterruptMessage>) -> Self {
        Self(client)
    }

    pub fn into_inner(self) -> RPCClient<InterruptMessage> {
        self.0
    }

    pub const fn client(&mut self) -> &mut RPCClient<InterruptMessage> {
        &mut self.0
    }

    pub fn wellknown() -> Self {
        Self(RPCClient::new(
            get_and_connect_service("INTERRUPTS").unwrap(),
        ))
    }

    pub fn subscribe(&mut self, vector: Vector) -> Interrupt {
        let mut c = Subscribe::new_req();
        c.init().set_vector(vector);
        let mut reply = self.client().send(&c.build()).unwrap();
        let mut handles = reply.take_handles_rpc();
        let mut reply = reply.get_reply().unwrap();
        let reply = reply.get_message().unwrap();

        let handle = handles.take_handle(reply.get_handle().unwrap());
        Interrupt::from_handle(handle.unwrap())
    }
}
