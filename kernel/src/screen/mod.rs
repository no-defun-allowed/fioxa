#[macro_use]
pub mod gop;
pub mod mouse;
pub mod psf1;
pub mod server;

use core::sync::atomic::{AtomicBool, Ordering};
static KERNEL_DRAWS_SCREEN: AtomicBool = AtomicBool::new(true);

fn is_kernel_drawing_screen() -> bool {
    KERNEL_DRAWS_SCREEN.load(Ordering::SeqCst)
}
fn set_kernel_drawing_screen(value: bool) {
    KERNEL_DRAWS_SCREEN.store(value, Ordering::SeqCst)
}
