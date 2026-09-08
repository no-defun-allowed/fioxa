fn scale(n: u16, size: u16, low: f64, high: f64) -> f64 {
    n as f64 / size as f64 * (high - low) + low
}

fn pixel(x0: f64, y0: f64, limit: usize) -> usize {
    let mut i = 0;
    let (mut xi, mut yi) = (0.0, 0.0);
    let (mut xsq, mut ysq) = (0.0, 0.0);
    while xi * xi + yi * yi < 4.0 && i < limit {
        yi = 2.0 * xi * yi + y0;
        xi = xsq - ysq + x0;
        xsq = xi * xi; ysq = yi * yi;
        i += 1;
    }
    i
}

fn colour(i: usize, limit: usize) -> (u8, u8, u8) {
    if i == limit {
        (0, 0, 0)
    } else {
        let i = libm::sqrtf(i as f32);
        let r = 128.0 + libm::sinf(i * 0.35) * 127.0;
        let g = 128.0 + libm::sinf(i * 0.70) * 127.0;
        let b = 128.0 + libm::sinf(i * 1.05) * 127.0;
        (r as u8, g as u8, b as u8)
    }
}

pub fn mandel(width: u16, height: u16, limit: usize,
              x1: f64, x2: f64, y1: f64, y2: f64,
              this_x: u16, this_y: u16) -> (u8, u8, u8) {
    let x0 = scale(this_x, width, x1, x2);
    let y0 = scale(this_y, height, y1, y2);
    let i = pixel(x0, y0, limit);
    colour(i, limit)
}
