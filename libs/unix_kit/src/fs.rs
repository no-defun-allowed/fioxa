// I/O stuff

use alloc::{boxed::Box, collections::VecDeque, string::String, vec, vec::Vec};
use core::ffi::{c_char, c_int};
use core::convert::TryInto;
use fioxa_rpc::{client::RPCClient, fs::{Read, Size}, fs_capnp::FileMessage};
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

// Shims for input and output channels.
enum LineBuffer {
    Done(Vec<u8>),
    Building(String),
    Eof,
}

struct Input {
    chan: Channel,
    this_line: LineBuffer,
    to_process: VecDeque<u8>,
}

impl Input {
    fn new(chan: Channel) -> Self {
        Self {
            chan,
            this_line: LineBuffer::Building(String::new()),
            to_process: VecDeque::new(),
        }
    }
}

impl File for Input {
    fn is_a_tty(&self) -> bool { true }
    fn read(&mut self, buf: *mut u8, count: usize) -> Result<usize, c_int> {
        // Read a new line if we don't have one.
        while let &mut LineBuffer::Building(ref mut buffer) = &mut self.this_line {
            while self.to_process.is_empty() {
                let mut new = vec![];
                if self.chan.read::<0>(&mut new, true, true).is_err() {
                    return Err(EIO);
                }
                self.to_process.extend(new);
            }
            while let Some(c) = self.to_process.pop_front() {
                let c = c as char; // not really, no
                match c {
                    '\x04' => {
                        if buffer.is_empty() {
                            self.this_line = LineBuffer::Eof;
                            break;
                        } else {
                            print!("\x07");
                        }
                    },
                    '\x08' => if buffer.pop().is_some() {
                        print!("\x08");
                    },
                    '\n' => {
                        println!();
                        buffer.push('\n');
                        self.this_line = LineBuffer::Done(core::mem::take(buffer).into_bytes());
                        break;
                    },
                    _ => {
                        print!("{c}");
                        buffer.push(c);
                    },
                }
            }
        }
        match *&mut self.this_line {
            LineBuffer::Building(_) => unreachable!(),
            LineBuffer::Done(ref mut bytes) => {
                let len = bytes.len().min(count);
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        bytes.as_ptr(),
                        buf,
                        len,
                    );
                }
                bytes.drain(0..len);
                if bytes.is_empty() {
                    self.this_line = LineBuffer::Building(String::new());
                }
                Ok(len)
            },
            LineBuffer::Eof => Ok(0),
        }
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

// Actual files backed by actual file systems.
type FileClient = RPCClient<FileMessage>;
struct ActualFile { client: FileClient, position: u64 }

impl ActualFile {
    pub fn len(&mut self) -> Option<u64> {
        let mut req = Size::new_req();
        req.init();
        let r = self.client.send(&req.build()).unwrap();
        let mut r = r.get_reply().unwrap();
        Some(r.get_message().unwrap().get_size())
    }
    pub fn read_absolute(&mut self, start: u64, len: u32) -> Option<Vec<u8>> {
        let mut req = Read::new_req();
        let mut b = req.init();
        b.set_offset(start);
        b.set_len(len);
        let r = self.client.send(&req.build()).unwrap();
        let mut r = r.get_reply().unwrap();
        Some(r.get_message().unwrap().get_data().unwrap().to_vec())
    }
}

impl File for ActualFile {
    fn is_a_tty(&self) -> bool { false }
    fn read(&mut self, buf: *mut u8, count: usize) -> Result<usize, c_int> {
        if let Some(read) = self.read_absolute(self.position, count.try_into().unwrap()) {
            unsafe {
                core::ptr::copy_nonoverlapping(read.as_ptr(), buf, read.len());
            }
            Ok(read.len())
        } else {
            Err(EIO)
        }
    }
    fn seek(&mut self, offset: i64, by: Seek) -> Result<i64, c_int> {
        let new_position = match by {
            Seek::Set => offset,
            Seek::End => {
                if let Some(len) = self.len() {
                    (len as i64) + offset
                } else {
                    return Err(EIO)
                }
            },
            Seek::Cur => (self.position as i64) + offset,
        };
        if new_position < 0 {
            Err(EINVAL)
        } else {
            self.position = new_position as u64;
            Ok(new_position)
        }
    }
    fn write(&mut self, buf: *const u8, count: usize) -> Result<(), c_int> {
        unimplemented!()
    }
}

static FILE_DESCRIPTORS: Lazy<Mutex<Vec<Option<Box<dyn File>>>>> = Lazy::new(|| {
    Mutex::new(alloc::vec![
        Some(Box::new(Input::new(userspace::print::STDIN_CHANNEL.clone()))),
        Some(Box::new(Output::new(&userspace::print::WRITER_STDOUT))),
        Some(Box::new(Output::new(&userspace::print::WRITER_STDERR))),
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
pub extern "C" fn fioxa_open(name: *const c_char, _flags: c_int, _mode: c_int, fd: *mut c_int) -> c_int {
    let file = unimplemented!(); // ActualFile::new(name);
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
