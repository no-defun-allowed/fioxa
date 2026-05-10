#![no_std]
#![no_main]

use core::time::Duration;

use fioxa_rpc::{
    client::RPCClient,
    fs::{GetChildren, add_path, stat_by_path, tree},
    fs_capnp,
    service::{connect_service, get_services},
};
use kernel_userspace::{
    channel::Channel,
    message::MessageHandle,
    mutex::Mutex,
    process::INIT_HANDLE_CHANNEL,
    sys::syscall::{sys_echo, sys_exit, sys_process_spawn_thread, sys_sleep},
};

extern crate alloc;
#[macro_use]
extern crate userspace;
extern crate userspace_slaballoc;

use alloc::{
    borrow::ToOwned, boxed::Box, collections::VecDeque, string::String, sync::Arc, vec::Vec,
};
use userspace::print::{STDERR_CHANNEL, STDIN_CHANNEL, STDOUT_CHANNEL, WRITER_STDOUT};

init_userspace!(main);

pub fn main() {
    let mut cwd: String = "/".to_owned();

    let mut input_history: VecDeque<Box<str>> = VecDeque::new();

    let mut input_buf = String::new();
    let mut input = input_buf.chars();

    let filesystems: Arc<Mutex<Vec<Channel>>> = Default::default();

    sys_process_spawn_thread({
        let filesystems = filesystems.clone();
        move || {
            get_services("FS", true, |chan| {
                filesystems.lock().push(chan);
            })
            .unwrap();
        }
    });

    let mut current_fs: Option<usize> = None;

    loop {
        match current_fs {
            Some(id) => print!("{id}:{cwd} "),
            None => print!(":{cwd} "),
        }

        let mut curr_line = String::new();
        let mut history_pos: usize = 0;

        loop {
            let Some(c) = input.next() else {
                unsafe {
                    STDIN_CHANNEL
                        .read::<0>(input_buf.as_mut_vec(), true, true)
                        .unwrap()
                };
                input = input_buf.chars();
                continue;
            };
            if c == '\n' {
                if !curr_line.is_empty() {
                    input_history.push_front(curr_line.clone().into());
                    if input_history.len() > 1000 {
                        input_history.pop_back();
                    }
                }
                println!();
                break;
            } else if c == '\x08' {
                if curr_line.pop().is_some() {
                    print!("\x08");
                }
            } else if c == '\u{02193}' {
                history_pos = history_pos.saturating_sub(1);
                while curr_line.pop().is_some() {
                    print!("\x08");
                }
                if history_pos > 0
                    && let Some(chr) = input_history.get(history_pos - 1)
                {
                    curr_line.push_str(chr);
                    print!("{curr_line}")
                }
            } else if c == '\u{02191}' {
                if let Some(chr) = input_history.get(history_pos) {
                    history_pos += 1;
                    while curr_line.pop().is_some() {
                        print!("\x08");
                    }
                    curr_line.push_str(chr);
                    print!("{curr_line}")
                }
            } else {
                curr_line.push(c);
                print!("{c}");
            }
        }

        let (command, rest) = curr_line
            .trim()
            .split_once(' ')
            .unwrap_or((curr_line.as_str(), ""));
        match command {
            "" => (),
            "pwd" => println!("{cwd}"),
            "echo" => println!("{rest}"),
            "disk" => {
                let c = rest.trim();
                let num = c.parse::<usize>();
                let fs_len = filesystems.lock().len();
                match num {
                    Ok(num) => {
                        if num < fs_len {
                            current_fs = Some(num);
                        } else {
                            println!("Disk {num} not in range 0..{fs_len}");
                        }
                    }
                    Err(_) => {
                        println!("Disks: 0..{fs_len}")
                    }
                }
            }
            "ls" => {
                let Some(fs_id) = current_fs else {
                    println!("No disk selected");
                    continue;
                };

                let fs = filesystems.lock().get(fs_id).cloned().unwrap();

                let path = add_path(&cwd, rest);

                match stat_by_path(fs, &path).unwrap() {
                    fioxa_rpc::fs::StatResult::None => println!("Invalid path"),
                    fioxa_rpc::fs::StatResult::File(_) => println!("This is a file"),
                    fioxa_rpc::fs::StatResult::Folder(channel) => {
                        let mut folder = RPCClient::<fs_capnp::FolderMessage>::new(
                            connect_service(&channel).unwrap(),
                        );

                        let mut req = GetChildren::new_req();
                        req.init();
                        let r = folder.send(&req.build()).unwrap();
                        let mut r = r.get_reply().unwrap();
                        let r = r.get_message().unwrap();

                        if r.has_entries() {
                            let mut names: Vec<_> = r
                                .get_entries()
                                .unwrap()
                                .iter()
                                .map(|e| e.get_name().unwrap().to_str().unwrap())
                                .collect();

                            numeric_sort::sort_unstable(&mut names);

                            for child in names {
                                println!("{child}")
                            }
                        }
                    }
                }
            }
            "tree" => {
                let Some(fs_id) = current_fs else {
                    println!("No disk selected");
                    continue;
                };

                let fs = filesystems.lock().get(fs_id).cloned().unwrap();

                let path = add_path(&cwd, rest);

                match stat_by_path(fs, &path).unwrap() {
                    fioxa_rpc::fs::StatResult::None => println!("Invalid path"),
                    fioxa_rpc::fs::StatResult::File(_) => println!("This is a file"),
                    fioxa_rpc::fs::StatResult::Folder(channel) => {
                        let stdout = &mut *WRITER_STDOUT.lock();
                        tree(stdout, &channel, String::new()).unwrap();
                    }
                }
            }
            "cd" => {
                let Some(fs_id) = current_fs else {
                    println!("No disk selected");
                    continue;
                };

                let fs = filesystems.lock().get(fs_id).cloned().unwrap();

                let path = add_path(&cwd, rest);

                match stat_by_path(fs, &path).unwrap() {
                    fioxa_rpc::fs::StatResult::None => println!("Invalid path"),
                    fioxa_rpc::fs::StatResult::File(_) => println!("This is a file"),
                    fioxa_rpc::fs::StatResult::Folder(_) => {
                        cwd = path;
                    }
                }
            }
            "cat" => {
                for file in rest.split_ascii_whitespace() {
                    let Some(fs_id) = current_fs else {
                        println!("No disk selected");
                        continue;
                    };

                    let fs = filesystems.lock().get(fs_id).cloned().unwrap();

                    let path = add_path(&cwd, file);

                    match stat_by_path(fs, &path).unwrap() {
                        fioxa_rpc::fs::StatResult::None => println!("Invalid path"),
                        fioxa_rpc::fs::StatResult::Folder(_) => println!("This is a folder"),
                        fioxa_rpc::fs::StatResult::File(file) => {
                            let mut file = RPCClient::<fioxa_rpc::fs_capnp::FileMessage>::new(
                                connect_service(&file).unwrap(),
                            );

                            let mut req = fioxa_rpc::fs::Size::new_req();
                            req.init();
                            let r = file.send(&req.build()).unwrap();
                            let mut r = r.get_reply().unwrap();
                            let length = r.get_message().unwrap().get_size() as usize;

                            const READ_SIZE: usize = 64 * 1024;
                            for start in (0..length).step_by(READ_SIZE) {
                                let len = (length - start).min(READ_SIZE);

                                let mut req = fioxa_rpc::fs::Read::new_req();
                                let mut b = req.init();
                                b.set_offset(start as u64);
                                b.set_len(len as u32);

                                let r = file.send(&req.build()).unwrap();
                                let mut r = r.get_reply().unwrap();
                                let data = r.get_message().unwrap().get_data().unwrap();

                                WRITER_STDOUT.lock().write_raw(data).unwrap();
                            }
                        }
                    }
                }
            }
            "write" => {
                let Some((file, data)) = rest.split_once(" ") else {
                    println!("bad args");
                    continue;
                };
                let Some(fs_id) = current_fs else {
                    println!("No disk selected");
                    continue;
                };

                let fs = filesystems.lock().get(fs_id).cloned().unwrap();

                let path = add_path(&cwd, file);

                match stat_by_path(fs, &path).unwrap() {
                    fioxa_rpc::fs::StatResult::None => println!("Invalid path"),
                    fioxa_rpc::fs::StatResult::Folder(_) => println!("This is a folder"),
                    fioxa_rpc::fs::StatResult::File(file) => {
                        let mut file = RPCClient::<fioxa_rpc::fs_capnp::FileMessage>::new(
                            connect_service(&file).unwrap(),
                        );
                        let mut req = fioxa_rpc::fs::Write::new_req();
                        let mut b = req.init();
                        b.set_offset(0);
                        b.set_data(data.as_bytes());
                        file.send(&req.build()).unwrap();
                    }
                }
            }
            "append" => {
                let Some((file, data)) = rest.split_once(" ") else {
                    println!("bad args");
                    continue;
                };
                let Some(fs_id) = current_fs else {
                    println!("No disk selected");
                    continue;
                };

                let fs = filesystems.lock().get(fs_id).cloned().unwrap();

                let path = add_path(&cwd, file);

                match stat_by_path(fs, &path).unwrap() {
                    fioxa_rpc::fs::StatResult::None => println!("Invalid path"),
                    fioxa_rpc::fs::StatResult::Folder(_) => println!("This is a folder"),
                    fioxa_rpc::fs::StatResult::File(file) => {
                        let mut file = RPCClient::<fioxa_rpc::fs_capnp::FileMessage>::new(
                            connect_service(&file).unwrap(),
                        );
                        let mut req = fioxa_rpc::fs::Size::new_req();
                        req.init();
                        let r = file.send(&req.build()).unwrap();
                        let size = r.get_reply().unwrap().get_message().unwrap().get_size();

                        let mut req = fioxa_rpc::fs::Write::new_req();
                        let mut b = req.init();
                        b.set_offset(size);
                        b.set_data(data.as_bytes());
                        file.send(&req.build()).unwrap();
                    }
                }
            }
            "exec" => {
                let Some(fs_id) = current_fs else {
                    println!("No disk selected");
                    continue;
                };

                let (prog, args) = rest.split_once(' ').unwrap_or((rest, ""));

                let args = MessageHandle::create(args.as_bytes());

                let fs = filesystems.lock().get(fs_id).cloned().unwrap();

                let path = add_path(&cwd, prog);

                let proc = match stat_by_path(fs, &path).unwrap() {
                    fioxa_rpc::fs::StatResult::None => {
                        println!("Invalid path");
                        continue;
                    }
                    fioxa_rpc::fs::StatResult::Folder(_) => {
                        println!("This is a folder");
                        continue;
                    }
                    fioxa_rpc::fs::StatResult::File(channel) => {
                        fioxa_rpc::elf::ElfClient::wellknown().spawn(
                            channel.handle(),
                            &[
                                INIT_HANDLE_CHANNEL.handle(),
                                STDIN_CHANNEL.handle(),
                                STDOUT_CHANNEL.handle(),
                                STDERR_CHANNEL.handle(),
                                args.handle(),
                            ],
                        )
                    }
                };

                match proc {
                    Ok(mut p) => {
                        p.blocking_exit_code();
                    }
                    Err(e) => println!("failed spawning: {e:?}"),
                }
            }
            // "uptime" => {
            //     let mut uptime = time::uptime() / 1000;
            //     let seconds = uptime % 60;
            //     uptime /= 60;
            //     let minutes = uptime % 60;
            //     uptime /= 60;
            //     println!("Up: {:02}:{:02}:{:02}", uptime, minutes, seconds)
            // }
            "sleep" => match rest.parse::<u64>() {
                Ok(n) => {
                    let act = sys_sleep(Duration::from_millis(n));
                    println!("sleep: slept for {act:?}");
                }
                Err(e) => println!("sleep: {e:?}"),
            },
            "test" => {
                let test: [u8; 6] = [1, 2, 45, 29, 23, 45];

                let handle = MessageHandle::create(&test);
                let h2 = handle.clone();
                drop(handle);

                let res = h2.read_vec();
                assert_eq!(test, *res);

                for i in 0..0x1000 {
                    assert_eq!(sys_echo(i), i);
                }

                println!("Passed test");
            }
            "exit" => {
                sys_exit();
            }
            _ => {
                println!("{command}: command not found")
            }
        }
    }
}
