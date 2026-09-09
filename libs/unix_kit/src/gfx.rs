use fioxa_rpc::{fb, service::get_and_connect_service};
use kernel_userspace::{channel::Channel, mutex::Mutex};

struct Gfx {
    client: fb::FBClient,
    framebuffer: fb::Framebuffer,
    map: *mut u32,
}
unsafe impl Send for Gfx { }

static GFX: Mutex<Option<Gfx>> = Mutex::new(None);

#[repr(C)]
pub struct Shape {
    width: u16,
    height: u16,
}

#[unsafe(no_mangle)]
pub extern "C" fn gfx_setup() -> Shape {
    let mut lock = GFX.lock();
    let mut client = fb::FBClient::connect();
    let framebuffer = client.get_framebuffer();
    let map = framebuffer.map() as *mut u32;
    client.acquire();
    let shape = Shape { width: framebuffer.width, height: framebuffer.height };
    *lock = Some(Gfx { client, framebuffer, map });
    shape
}

#[unsafe(no_mangle)]
pub extern "C" fn gfx_draw_image(fb: *mut u32, width: u32, height: u32) {
    let lock = GFX.lock();
    let Some(ref gfx) = *lock else {
        panic!("call gfx_setup before gfx_draw_image!");
    };
    for y in 0..height {
        unsafe {
            let target_start = gfx.map.add((gfx.framebuffer.stride as u32 * y) as usize);
            let source_start = fb.add((width * y) as usize);
            core::ptr::copy_nonoverlapping(source_start, target_start, width as usize);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gfx_draw_image_scaled(fb: *mut u32, width: u32, height: u32, scale: u32) {
    let lock = GFX.lock();
    let Some(ref gfx) = *lock else {
        panic!("call gfx_setup before gfx_draw_image_scaled!");
    };
    unsafe {
        for y in 0..height {
            for sy in 0..scale {
                let target_start = gfx.map.add((gfx.framebuffer.stride as u32 * ((scale * y) + sy)) as usize);
                let source_start = fb.add((width * y) as usize);
                for x in 0..width {
                    for sx in 0..scale {
                        *target_start.add((scale * x + sx) as usize) = *source_start.add(x as usize);
                    }
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gfx_destroy() {
    let mut lock = GFX.lock();
    let Some(ref mut gfx) = *lock else {
        panic!("call gfx_setup before gfx_destroy");
    };
    gfx.client.release();
    *lock = None;
}

static KEYBOARD: Mutex<Option<Channel>> = Mutex::new(None);

#[unsafe(no_mangle)]
pub extern "C" fn keyboard_setup() {
    let mut lock = KEYBOARD.lock();
    *lock = Some(get_and_connect_service("INPUT:KB").unwrap());
}

#[repr(C)]
pub struct KeyboardPoll {
    state: u8,
    key: u32,
}

use input::keyboard::virtual_code::*;
fn translate_key(c: VirtualKeyCode) -> u32 {
    match c {
        VirtualKeyCode::Letter(letter) => match letter {
            Letter::X => 'X' as u32,
            Letter::C => 'C' as u32,
            _ => 0x123,
        },
        VirtualKeyCode::Control(key) => match key {
            Control::Escape => 0x1B,
            Control::ArrowLeft => 0x2190,
            Control::ArrowUp => 0x2191,
            Control::ArrowRight => 0x2192,
            Control::ArrowDown => 0x2193,
            _ => 0x123,
        },
        _ => 0x123,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn keyboard_poll() -> KeyboardPoll {
    let lock = KEYBOARD.lock();
    let Some(ref chan) = *lock else {
        panic!("call keyboard_setup before keyboard_poll");
    };
    use input::keyboard::KeyboardEvent;
    use kernel_userspace::input::InputServiceMessage;
    match chan.read_val::<1, InputServiceMessage>(false) {
        Ok((event, _)) => match event {
            InputServiceMessage::KeyboardEvent(KeyboardEvent::Down(c)) => KeyboardPoll { state: 1, key: translate_key(c) },
            InputServiceMessage::KeyboardEvent(KeyboardEvent::Up(c)) => KeyboardPoll { state: 2, key: translate_key(c) },
            _ => unreachable!(),
        },
        Err(_) => KeyboardPoll { state: 0, key: 0 },
    }
}
