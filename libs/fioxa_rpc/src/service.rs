use alloc::vec::Vec;
use kernel_userspace::{
    channel::Channel,
    handle::Handle,
    process::INIT_HANDLE_CHANNEL,
    sys::types::{KernelObjectType, ObjectSignal, SyscallError},
};

use crate::{RPCHandleBuilder, client::RPCClient};

#[must_use]
pub struct ServiceExecutor<I: Fn(Channel)> {
    channel: Channel,
    service: I,
}

impl<I: Fn(Channel)> ServiceExecutor<I> {
    pub fn from_channel(channel: Channel, service: I) -> Self {
        Self { channel, service }
    }

    pub fn with_name(name: &str, service: I) -> Self {
        let (l, r) = Channel::new();

        register_service(name, r.into_inner()).unwrap();

        Self {
            channel: l,
            service,
        }
    }

    pub fn run(&mut self) -> Result<(), SyscallError> {
        loop {
            match run_service_iter(&self.channel, &self.service) {
                Ok(()) => (),
                Err(SyscallError::ChannelClosed) => return Ok(()),
                Err(SyscallError::ChannelEmpty) => {
                    self.channel.handle().wait(ObjectSignal::all())?;
                }
                e => return e,
            }
        }
    }
}

pub fn run_service_iter(chan: &Channel, mut f: impl FnMut(Channel)) -> Result<(), SyscallError> {
    let mut vec = Vec::new();
    let handles = chan.read::<32>(&mut vec, false, false)?;

    for handle in handles {
        let ty = handle.get_type();
        if ty == KernelObjectType::Channel {
            f(Channel::from_handle(handle));
        } else {
            // TODO: Warn?
        }
    }

    Ok(())
}

pub fn init_handle_registry() -> RPCClient<crate::registry_capnp::RegistryMessage> {
    RPCClient::new(connect_service(&INIT_HANDLE_CHANNEL).unwrap())
}

pub fn get_and_connect_service(name: &str) -> Result<Channel, SyscallError> {
    let s = get_service(name, true).ok_or(SyscallError::UnknownHandle)?;
    connect_service(&s)
}

pub fn connect_service(handle: &Channel) -> Result<Channel, SyscallError> {
    let (left, right) = Channel::new();
    loop {
        match handle.write(&[], &[**right.handle()]) {
            Ok(()) => return Ok(left),
            Err(SyscallError::ChannelFull) => handle.handle().wait(ObjectSignal::all()).unwrap(),
            Err(e) => return Err(e),
        };
    }
}

pub fn get_service(name: &str, blocking: bool) -> Option<Channel> {
    let mut get = crate::registery::Get::new_req();
    let mut builder = get.init();
    builder.set_name(name);
    builder.init_mode().init_any().set_blocking(blocking);

    let mut r = init_handle_registry();
    let mut reply = r.send(&get.build()).ok()?;

    let mut handles = reply.take_handles_rpc();
    let mut reply = reply.get_reply().ok()?;
    let reply = reply.get_message().ok()?;

    let h = if reply.has_entries() {
        handles
            .take_handle(reply.get_entries().unwrap().get(0))
            .unwrap()
    } else if reply.has_extra() {
        let chan = Channel::from_handle(handles.take_handle(reply.get_extra().unwrap()).unwrap());
        let mut v = chan.read::<1>(&mut Vec::new(), false, true).unwrap();
        v.remove(0)
    } else {
        return None;
    };

    Some(Channel::from_handle(h))
}

pub fn register_service(name: &str, handle: Handle) -> Result<(), SyscallError> {
    let mut call = crate::registery::Register::new_req();
    let mut handles = RPCHandleBuilder::default();
    let mut builder = call.init();
    builder.set_name(name);
    handles.add(builder.init_handle(), handle);

    let mut r = init_handle_registry();
    r.send(&call.build_handles(&handles))?;
    Ok(())
}

pub fn get_services(
    name: &str,
    cont: bool,
    mut f: impl FnMut(Channel),
) -> Result<(), SyscallError> {
    let mut r = init_handle_registry();

    let mut call = crate::registery::Get::new_req();
    let mut builder = call.init();
    builder.set_name(name);
    builder.init_mode().init_stream().set_continue(cont);

    let mut reply = r.send(&call.build())?;
    let mut handles = reply.take_handles_rpc();
    let mut reply = reply.get_reply().unwrap();
    let reply = reply.get_message().unwrap();

    if reply.has_entries() {
        for entry in reply.get_entries().unwrap() {
            f(Channel::from_handle(handles.take_handle(entry).unwrap()));
        }
    } else if reply.has_extra() {
        let chan = Channel::from_handle(handles.take_handle(reply.get_extra().unwrap()).unwrap());

        while let Ok(v) = chan.read::<1>(&mut Vec::new(), false, true) {
            for h in v {
                f(Channel::from_handle(h))
            }
        }
    }

    Ok(())
}
