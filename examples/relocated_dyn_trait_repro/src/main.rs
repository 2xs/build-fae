#![no_std]
#![no_main]

use core::panic::PanicInfo;

pub trait Handler: Sync {
    fn run(&self) -> u32;
}

#[repr(C)]
pub struct Impl {
    pub value: u32,
}

unsafe impl Sync for Impl {}

impl Handler for Impl {
    #[inline(never)]
    fn run(&self) -> u32 {
        unsafe { core::ptr::read_volatile(core::ptr::addr_of!(self.value)) }
    }
}

#[used]
pub static HANDLER_IMPL: Impl = Impl { value: 42 };

#[used]
pub static DESCRIPTOR: &'static dyn Handler = &HANDLER_IMPL;

#[unsafe(no_mangle)]
pub extern "C" fn start() -> u32 {
    let handler = core::hint::black_box(gen_dyn());
    let value = core::hint::black_box(use_dyn(handler));
    if value != 42 {
        panic!();
    }
    value
}

#[inline(never)]
fn gen_dyn() -> &'static dyn Handler {
    core::hint::black_box(DESCRIPTOR)
}

#[inline(never)]
fn use_dyn(handler: &dyn Handler) -> u32 {
    let handler = core::hint::black_box(handler);
    core::hint::black_box(handler.run())
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    unsafe {
        core::arch::asm!("udf #0", options(noreturn));
    }
}
