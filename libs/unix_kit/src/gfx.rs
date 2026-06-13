use alloc::format;
use alloc::string::{String, ToString};
use base64::prelude::*;
use itertools::Itertools;

#[unsafe(no_mangle)]
pub extern "C" fn gfx_draw_image(fb: *mut u32, width: u32, height: u32) {
    let size = (4 * width * height) as usize;
    let data = unsafe { core::slice::from_raw_parts(fb as *const u8, size) };
    let spec = nopng::ImageSpec::new(width, height, nopng::PixelFormat::Rgba8);
    let png = nopng::encode_image(&spec, data).unwrap();
    let encoded = BASE64_STANDARD.encode(png);
    let mut first = true;
    for chunk in &encoded.chars().chunks(4096) {
        let c: String = chunk.collect();
        if first {
            print!("\x1B_Ga=T,f=100,m=1;{c}\x1B\\");
        } else {
            print!("\x1B_Gm=1;{c}\x1B\\");
        }
        first = false;
    }
    print!("\x1B_Gm=0;\x1B\\");
}
