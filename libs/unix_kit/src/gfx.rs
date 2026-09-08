use fioxa_rpc::fb;
use kernel_userspace::mutex::Mutex;

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
pub extern "C" fn gfx_destroy() {
    let mut lock = GFX.lock();
    let Some(ref mut gfx) = *lock else {
        panic!("call gfx_setup before gfx_destroy");
    };
    gfx.client.release();
    *lock = None;
}
