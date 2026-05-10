use alloc::{borrow::ToOwned, string::String, vec::Vec};
use kernel_userspace::{channel::Channel, sys::types::SyscallError};

use crate::{
    client::RPCClient,
    fs_capnp::{self, FileType},
    service::connect_service,
};

crate::generate_rpc!(crate::fs_capnp::FolderMessage, FolderService;
    GetChildren @ GetChildren @ get_children(fs_capnp::folder_get_children::Owned) -> fs_capnp::folder_got_children::Owned;
    Open @ FolderOpen @ open(fs_capnp::folder_open::Owned) -> fs_capnp::folder_opened::Owned;
);

crate::generate_rpc!(fs_capnp::FileMessage, FileService;
    Size @ Size @ size(fs_capnp::file_size::Owned) -> fs_capnp::file_size_read::Owned;
    Read @ Read @ read(fs_capnp::file_read::Owned) -> fs_capnp::file_data::Owned;
    Write @ Write @ write(fs_capnp::file_write::Owned) -> fs_capnp::file_write_resp::Owned;
);

pub fn add_path(folder: &str, file: &str) -> String {
    if file.starts_with('/') {
        return file.to_owned();
    }

    let mut path: Vec<&str> = folder.split('/').filter(|a| !a.is_empty()).collect();

    for sect in file.split('/') {
        if sect.is_empty() || sect == "." {
            continue;
        } else if sect == ".." {
            path.pop();
        } else {
            path.push(sect)
        }
    }

    "/".to_owned() + path.join("/").as_str()
}

pub enum StatResult {
    None,
    Folder(Channel),
    File(Channel),
}

pub fn stat_by_path(root: Channel, path: &str) -> Result<StatResult, SyscallError> {
    let mut root = StatResult::Folder(root);
    for sect in path.split('/') {
        if sect.is_empty() {
            continue;
        }

        let mut folder = match root {
            StatResult::File(_) | StatResult::None => return Ok(StatResult::None),
            StatResult::Folder(channel) => {
                RPCClient::<fs_capnp::FolderMessage>::new(connect_service(&channel)?)
            }
        };
        let mut c = FolderOpen::new_req();
        c.init().set_name(sect);
        let mut r = folder.send(&c.build()).unwrap();
        let mut h = r.take_handles_rpc();
        let mut r = r.get_reply().unwrap();
        let r = r.get_message().unwrap();
        root = match r.get_type().unwrap() {
            fs_capnp::FileType::None => StatResult::None,
            fs_capnp::FileType::File => StatResult::File(Channel::from_handle(
                h.take_handle(r.get_capability().unwrap()).unwrap(),
            )),
            fs_capnp::FileType::Folder => StatResult::Folder(Channel::from_handle(
                h.take_handle(r.get_capability().unwrap()).unwrap(),
            )),
        };
    }
    Ok(root)
}

pub fn tree(
    writer: &mut impl core::fmt::Write,
    root: &Channel,
    prefix: String,
) -> Result<(), core::fmt::Error> {
    let mut folder = RPCClient::<fs_capnp::FolderMessage>::new(connect_service(root).unwrap());

    let mut c = GetChildren::new_req();
    c.init();
    let r = folder.send(&c.build()).unwrap();
    let mut r = r.get_reply().unwrap();
    let r = r.get_message().unwrap();

    if r.has_entries() {
        let mut names: Vec<_> = r
            .get_entries()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    e.get_name().unwrap().to_str().unwrap(),
                    e.get_type().unwrap(),
                )
            })
            .collect();

        names.sort_unstable_by(|(a, _), (b, _)| numeric_sort::cmp(a, b));

        let mut rec = |name, ty| {
            if ty == FileType::Folder {
                let mut c = FolderOpen::new_req();
                c.init().set_name(name);
                let mut r = folder.send(&c.build()).unwrap();
                let mut h = r.take_handles_rpc();
                let mut r = r.get_reply().unwrap();
                let r = r.get_message().unwrap();
                let h = h.take_handle(r.get_capability().unwrap()).unwrap();
                Some(h)
            } else {
                None
            }
        };

        if !names.is_empty() {
            for (name, ty) in names.iter().take(names.len() - 1) {
                writeln!(writer, "{}├── {}", prefix, name)?;
                if let Some(h) = rec(name, *ty) {
                    tree(writer, &Channel::from_handle(h), prefix.clone() + "│   ")?;
                }
            }

            let (name, ty) = names.last().unwrap();
            writeln!(writer, "{}└── {}", prefix, name)?;
            if let Some(h) = rec(name, *ty) {
                tree(writer, &Channel::from_handle(h), prefix.clone() + "    ")?;
            }
        }
    }

    Ok(())
}
