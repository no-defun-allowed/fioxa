use alloc::{vec::Vec, string::{String, ToString}};
use core::convert::From;
use fioxa_rpc::{
    client::RPCClient,
    fs::{stat_by_path, Read, Size, StatResult},
    service::{connect_service, get_services},
};
use kernel_userspace::{
    channel::Channel,
    mutex::Mutex,
    sys::types::SyscallError,
};

static FILESYSTEMS: Mutex<Vec<Channel>> = Mutex::new(alloc::vec![]);

pub fn probe_filesystems() {
    let mut fs = FILESYSTEMS.lock();
    get_services("FS", false, |chan| { fs.push(chan); }).unwrap();
}

pub struct Pathname {
    pub disk: usize,
    pub name: String,
}

impl Pathname {
    pub fn from_string(name: &str) -> Option<Self> {
        match name.split_once(":") {
            Some((disk, name)) => Some(Pathname {
                disk: disk.parse().ok()?,
                name: name.to_string(),
            }),
            None => Some(Pathname { disk: 0, name: name.to_string() }),
        }
    }
}

type FileClient = RPCClient::<fioxa_rpc::fs_capnp::FileMessage>;
pub struct File(FileClient);

#[derive(Debug)]
pub enum Error {
    Capnp(capnp::Error),
    InvalidDiskId,
    NotAFile,
    NotFound,
    Syscall(SyscallError),
}
impl From<SyscallError> for Error {
    fn from(e: SyscallError) -> Self { Error::Syscall(e) }
}
impl From<capnp::Error> for Error {
    fn from(e: capnp::Error) -> Self { Error::Capnp(e) }
}

impl File {
    pub fn open(path: &Pathname) -> Result<Self, Error> {
        let fs = FILESYSTEMS.lock();
        let Some(server) = fs.get(path.disk) else { return Err(Error::InvalidDiskId) };
        match stat_by_path(server.clone(), &path.name)? {
            StatResult::File(file) => Ok(File(FileClient::new(connect_service(&file)?))),
            StatResult::Folder(_) => Err(Error::NotAFile),
            StatResult::None => Err(Error::NotFound),
        }
    }
    
    pub fn len(&mut self) -> Result<usize, Error> {
        let mut req = Size::new_req();
        req.init();
        let r = self.0.send(&req.build())?;
        let mut r = r.get_reply()?;
        Ok(r.get_message().unwrap().get_size() as usize)
    }

    pub fn read(&mut self, start: usize, len: usize) -> Result<Vec<u8>, Error> {
        let mut req = Read::new_req();
        let mut b = req.init();
        b.set_offset(start as u64);
        b.set_len(len as u32);
        let r = self.0.send(&req.build())?;
        let mut r = r.get_reply()?;
        Ok(r.get_message().unwrap().get_data()?.to_vec())
    }
}
