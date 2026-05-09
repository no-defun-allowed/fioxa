use kernel_sys::{
    syscall::{sys_object_wait, sys_process_exit_code},
    types::{ObjectSignal, SyscallError},
};

use crate::{
    channel::Channel,
    handle::{FIRST_HANDLE, Handle},
};

pub static INIT_HANDLE_CHANNEL: Channel =
    unsafe { Channel::from_handle(Handle::from_id(FIRST_HANDLE)) };

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessHandle(Handle);

impl ProcessHandle {
    pub fn from_handle(handle: Handle) -> Self {
        Self(handle)
    }

    pub const fn handle(&self) -> &Handle {
        &self.0
    }

    pub fn into_inner(self) -> Handle {
        let Self(handle) = self;
        handle
    }

    pub fn get_exit_code(&self) -> Result<usize, SyscallError> {
        sys_process_exit_code(*self.0)
    }

    pub fn blocking_exit_code(&mut self) -> usize {
        loop {
            match self.get_exit_code() {
                Ok(val) => return val,
                Err(SyscallError::ProcessStillRunning) => {
                    sys_object_wait(*self.0, ObjectSignal::PROCESS_EXITED).unwrap();
                }
                Err(e) => panic!("unknown err {e:?}"),
            };
        }
    }
}
