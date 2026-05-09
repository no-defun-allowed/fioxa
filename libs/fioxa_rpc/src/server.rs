use core::marker::PhantomData;

use alloc::{format, vec::Vec};
use kernel_userspace::{
    channel::Channel,
    handle::Handle,
    sys::types::{Hid, SyscallError},
};

use crate::{RPCHandleBuilder, rpc_capnp};

pub struct RPCServer<H, T> {
    channel: Channel,
    handler: H,
    _t: PhantomData<T>,
}

impl<T, H: RPCServiceHandler<T>> RPCServer<H, T> {
    pub fn new(channel: Channel, handler: H) -> Self {
        Self {
            channel,
            handler,
            _t: PhantomData,
        }
    }

    pub fn run(&mut self) -> Result<(), capnp::Error> {
        let mut buf = Vec::with_capacity(0x1000);
        loop {
            let req_handles = match self.channel.read::<32>(&mut buf, true, true) {
                Err(SyscallError::ChannelClosed) => return Ok(()),
                r => r
                    .map_err(|e| capnp::Error::failed(format!("error reading: {e:?}")))?
                    .into_iter()
                    .collect(),
            };

            let req = capnp::serialize::read_message_from_flat_slice(
                &mut &*buf,
                capnp::message::DEFAULT_READER_OPTIONS,
            )?;

            let mut res = capnp::message::Builder::new_default();
            let mut res_handles = RPCHandleBuilder::default();

            self.handler
                .dispatch(req.get_root()?, req_handles, &mut res, &mut res_handles)?;

            let res = capnp::serialize::write_message_segments_to_words(&res);

            let handles: Vec<Hid> = res_handles.0.iter().map(|h| ***h).collect();

            self.channel
                .write(&res, &handles)
                .map_err(|e| capnp::Error::failed(format!("error writing: {e:?}")))?;

            // ensure they stay alive during the write
            drop(res_handles);
        }
    }
}

pub trait RPCServiceHandler<T> {
    fn dispatch<'a, A: capnp::message::Allocator>(
        &mut self,
        req: rpc_capnp::call::Reader<'a>,
        req_handles: Vec<Handle>,
        res: &mut capnp::message::Builder<A>,
        res_handles: &mut RPCHandleBuilder<'static>,
    ) -> Result<(), capnp::Error>;
}
