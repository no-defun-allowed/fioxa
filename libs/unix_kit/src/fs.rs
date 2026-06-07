// I/O stuff

use alloc::{boxed::Box, vec, vec::Vec};
use core::ffi::{c_char, c_int};
use core::convert::TryInto;
use kernel_userspace::{channel::Channel, mutex::Mutex, sys::types::SyscallError};
use userspace::print::Writer;
use spin::Lazy;

// Constants we need to fudge some syscalls with.
const ENOENT: c_int = 2;
const EIO: c_int = 5;
const EBADF: c_int = 9;
const EINVAL: c_int = 22;
const ENOTTY: c_int = 25;
const ESPIPE: c_int = 29;

const SEEK_SET: c_int = 0;
const SEEK_CUR: c_int = 1;
const SEEK_END: c_int = 2;

#[derive(Clone, Copy)]
enum Seek { Set, End, Cur }

trait File: Send {
    fn is_a_tty(&self) -> bool;
    fn read(&mut self, buf: *mut u8, count: usize) -> Result<usize, c_int>;
    fn seek(&mut self, offset: i64, by: Seek) -> Result<i64, c_int>;
    fn write(&mut self, buf: *const u8, count: usize) -> Result<(), c_int>;
}

struct Input { chan: Channel, buffer: Vec<u8> }

impl Input {
    fn new(chan: Channel) -> Self {
        Self { chan, buffer: vec![] }
    }
}

impl File for Input {
    fn is_a_tty(&self) -> bool { true }
    fn read(&mut self, buf: *mut u8, count: usize) -> Result<usize, c_int> {
        while self.buffer.is_empty() {
            let mut new = vec![];
            let Ok(_) = self.chan.read::<0>(&mut new, true, true) else {
                return Err(EIO);
            };
            self.buffer.append(&mut new);
        }
        let len = self.buffer.len().max(count);
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.buffer.as_ptr(),
                buf,
                len,
            );
        }
        self.buffer.drain(0..len);
        Ok(len)
    }
    fn seek(&mut self, _offset: i64, _by: Seek) -> Result<i64, c_int> { Err(ESPIPE) }
    fn write(&mut self, _buf: *const u8, _count: usize) -> Result<(), c_int> { Err(EBADF) }
}

struct Output { writer: &'static Mutex<Writer<'static>> }

impl Output {
    fn new(writer: &'static Mutex<Writer<'static>>) -> Self {
        Self { writer }
    }
}

impl File for Output {
    fn is_a_tty(&self) -> bool { true }
    fn read(&mut self, _buf: *mut u8, _count: usize) -> Result<usize, c_int> { Err(EBADF) }
    fn seek(&mut self, _offset: i64, _by: Seek) -> Result<i64, c_int> { Err(ESPIPE) }
    fn write(&mut self, buf: *const u8, count: usize) -> Result<(), c_int> {
        
        let buf = unsafe { core::slice::from_raw_parts(buf, count) };
        let _ = self.writer.lock().write_raw(buf);
        Ok(())
    }
}

static FILE_DESCRIPTORS: Lazy<Mutex<Vec<Option<Box<dyn File>>>>> = Lazy::new(|| {
    Mutex::new(alloc::vec![
        Some(Box::new(Input::new(userspace::print::STDIN_CHANNEL.clone()))),
        Some(Box::new(Output::new(&*userspace::print::WRITER_STDOUT))),
        Some(Box::new(Output::new(&*userspace::print::WRITER_STDERR))),
    ])
});

fn get_descriptor<T>(fd: c_int, hit: impl Fn(&mut Box<dyn File>) -> T, miss: T) -> T {
    let mut fds = FILE_DESCRIPTORS.lock();
    let Ok(fd): Result<usize, _> = fd.try_into() else { return miss };
    match fds.get_mut(fd) {
        Some(Some(file)) => hit(file),
        _ => miss,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_close(fd: c_int) -> c_int {
    let Ok(fd): Result<usize, _> = fd.try_into() else { return EBADF };
    let mut fds = FILE_DESCRIPTORS.lock();
     match fds.get_mut(fd) {
         Some(Some(_)) => {
             fds[fd] = None;
             0
         }
         _ => EBADF
     }
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_open(_name: *const c_char, _flags: c_int, _mode: c_int, fd: *mut c_int) -> c_int {
    return ENOENT;
    let file = unimplemented!();
    // Add it to the FDs
    let mut fds = FILE_DESCRIPTORS.lock();
    let new_fd = if let Some(reuse) = fds.iter().position(|f| f.is_none()) {
        fds[reuse] = file;
        reuse
    } else {
        fds.push(file);
        fds.len()
    };
    unsafe { *fd = new_fd as c_int };
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_read(fd: c_int, buf: *mut u8, count: usize, bytes_read: *mut usize) -> c_int {
    get_descriptor(fd, |file| {
        match file.read(buf, count) {
            Ok(read) => {
                unsafe { *bytes_read = read };
                0
            },
            Err(err) => err,
        }
    }, EBADF)
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_seek(fd: c_int, off: i64, whence: c_int, new_offset: *mut i64) -> c_int {
    let whence = match whence {
        SEEK_SET => Seek::Set,
        SEEK_CUR => Seek::Cur,
        SEEK_END => Seek::End,
        _ => return EINVAL,
    };
    get_descriptor(fd, |file| {
        match file.seek(off, whence) {
            Ok(offset) => {
                unsafe { *new_offset = offset };
                0
            },
            Err(err) => err,
        }
    }, EBADF)
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_isatty(fd: c_int) -> c_int {
    get_descriptor(fd, |file| {
        if file.is_a_tty() { 0 } else { ENOTTY }
    }, EBADF)
}

#[unsafe(no_mangle)]
pub extern "C" fn fioxa_write(fd: c_int, buf: *const u8, count: usize) -> c_int {
    get_descriptor(fd, |file| {
        match file.write(buf, count) {
            Ok(()) => 0,
            Err(err) => err,
        }
    }, EBADF)
}
