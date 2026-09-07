#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::slice;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn start(
    out_ptr: *mut u8,
    out_len: usize,
    payload_ptr: *const u8,
    payload_len: usize,
) -> u32 {
    let out = unsafe { slice::from_raw_parts_mut(out_ptr, out_len) };
    let payload = unsafe { slice::from_raw_parts(payload_ptr, payload_len) };
    copy_payload(out, payload);
    out.iter().fold(0u32, |acc, byte| acc + u32::from(*byte))
}

#[inline(never)]
fn copy_payload(out: &mut [u8], payload: &[u8]) {
    // Reproduces the Rust bounds/copy checks that emit read-only panic
    // location metadata containing absolute pointers to other ROM data.
    out[..payload.len()].copy_from_slice(payload);
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    unsafe {
        core::arch::asm!("udf #0", options(noreturn));
    }
}
