#![no_std]
#![no_main]

extern crate alloc;
#[macro_use]
extern crate userspace;
extern crate userspace_slaballoc;

mod fs;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::{format, vec};
use base64::prelude::*;
use itertools::Itertools;

init_userspace!(main);

struct Image {
    w: u32,
    h: u32,
    data: Vec<u8>,
}

fn draw(i: &Image) {
    let encoded = BASE64_STANDARD.encode(&i.data);
    let Image { w, h, .. } = i;
    let mut first = true;
    for chunk in &encoded.chars().chunks(4096) {
        let metadata = if first {
            format!("a=T,f=32,s={w},v={h},")
        } else {
            "".to_string()
        };
        first = false;
        let c: String = chunk.collect();
        print!("\x1B_G{metadata}m=1;{c}\x1B\\");
    }
    print!("\x1B_Gm=0;\x1B\\");
}

fn decode(data: &[u8]) -> Image {
    let header = minipng::decode_png_header(data).expect("bad PNG");
    let mut buffer = vec![0; header.required_bytes_rgba8bpc()];
    let mut image = minipng::decode_png(data, &mut buffer).expect("bad PNG");
    image.convert_to_rgba8bpc().expect("bad convert??");
    Image {
        w: image.width(),
        h: image.height(),
        data: image.pixels().to_vec(),
    }
}

pub fn main() {
    let args = userspace::ARGS.read_vec();
    let args = str::from_utf8(&args).unwrap();
    let pathname = fs::Pathname::from_string(args).expect("invalid pathname");

    fs::probe_filesystems();
    let mut file = fs::File::open(&pathname).expect("were file");
    let len = file.len().expect("could not len");
    let data = file
        .read(0, len.try_into().unwrap())
        .expect("could not read");
    let i = decode(&data);
    draw(&i);
}
