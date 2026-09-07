#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[repr(C)]
pub struct Descriptor {
    pub handler: unsafe extern "C" fn() -> u32,
}

unsafe impl Sync for Descriptor {}

#[used]
pub static DESCRIPTOR: Descriptor = Descriptor {
    handler,
};

#[unsafe(no_mangle)]
pub extern "C" fn start() -> u32 {
    let descriptor = core::hint::black_box(gen_dyn());
    let value = core::hint::black_box(use_dyn(descriptor));
    if value != 42 {
        panic!();
    }
    value
}

#[inline(never)]
fn gen_dyn() -> *const Descriptor {
    core::hint::black_box(&DESCRIPTOR)
}

#[inline(never)]
fn use_dyn(descriptor: *const Descriptor) -> u32 {
    let descriptor = core::hint::black_box(descriptor);
    let handler = unsafe { core::ptr::read_volatile(core::ptr::addr_of!((*descriptor).handler)) };
    unsafe { core::hint::black_box(handler()) }
}

unsafe extern "C" fn handler() -> u32 {
    42
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    unsafe {
        core::arch::asm!("udf #0", options(noreturn));
    }
}
