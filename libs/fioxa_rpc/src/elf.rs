use alloc::format;
use kernel_userspace::{handle::Handle, process::ProcessHandle};

use crate::{
    RPCHandleBuilder,
    client::RPCClient,
    elf_capnp::{self, ElfMessage},
    service::get_and_connect_service,
};

crate::generate_rpc!(elf_capnp::ElfMessage, Service;
    Spawn @ Spawn @ spawn(elf_capnp::spawn::Owned) -> elf_capnp::spawned::Owned;
);

pub struct ElfClient(RPCClient<ElfMessage>);

impl ElfClient {
    pub const fn new(client: RPCClient<ElfMessage>) -> Self {
        Self(client)
    }

    pub fn into_inner(self) -> RPCClient<ElfMessage> {
        self.0
    }

    pub const fn client(&mut self) -> &mut RPCClient<ElfMessage> {
        &mut self.0
    }

    pub fn wellknown() -> Self {
        Self(RPCClient::new(
            get_and_connect_service("ELF_LOADER").unwrap(),
        ))
    }

    pub fn spawn(
        &mut self,
        file: &Handle,
        refs: &[&Handle],
    ) -> Result<ProcessHandle, capnp::Error> {
        let mut c = Spawn::new_req();
        let mut handles = RPCHandleBuilder::new();
        let mut b = c.init();
        handles.add(b.reborrow().init_file(), file);

        let mut init = b.init_initial_refs(refs.len() as u32);
        for (i, h) in refs.iter().enumerate() {
            handles.add(init.reborrow().get(i as u32), *h);
        }

        let mut r = self
            .client()
            .send(&c.build_handles(&handles))
            .map_err(|e| capnp::Error::failed(format!("failed to send: {e:?}")))?;
        let mut h = r.take_handles_rpc();
        let mut r = r.get_reply()?;
        let r = r.get_message()?;
        let h = h
            .take_handle(r.get_handle()?)
            .ok_or_else(|| capnp::Error::failed("no handle".into()))?;
        Ok(ProcessHandle::from_handle(h))
    }
}
