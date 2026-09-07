#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn start() -> u32 {
    let cstring = core::hint::black_box(c"Hello cstring");
    let bytes = cstring.to_bytes();
    if bytes != b"Hello cstring" {
        panic!("cstring mismatch: got {} bytes", bytes.len());
    }
    bytes.len() as u32
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    unsafe {
        core::arch::asm!("udf #0", options(noreturn));
    }
}
