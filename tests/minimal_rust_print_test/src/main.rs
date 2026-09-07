#![no_std]
#![no_main]

use core::panic::PanicInfo;

use rust_xipfs_lib::stdriot::printf;

#[unsafe(no_mangle)]
pub static COUNTER: i32 = 7;

#[unsafe(no_mangle)]
pub static mut SCRATCH: i32 = 0;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn main() -> i32 {
    let message = c"Hello CString from minimal_rust_print! from message\n";
    printf(c"Hello from Minimal Rust Print\n");
    printf(message);
    0
}
