#![no_std]
#![no_main]

use core::panic::PanicInfo;

pub trait Handler: Sync {
    fn run(&self) -> u32;
}

#[repr(C)]
pub struct Impl;

unsafe impl Sync for Impl {}

impl Handler for Impl {
    fn run(&self) -> u32 {
        42
    }
}

#[used]
static HANDLER_IMPL: Impl = Impl;

#[used]
static DESCRIPTOR: &'static dyn Handler = &HANDLER_IMPL;

#[used]
static mut HEAP: [u8; 64] = [0; 64];

#[unsafe(no_mangle)]
pub extern "C" fn start() -> u32 {
    unsafe {
        let heap = core::ptr::addr_of_mut!(HEAP) as *mut u8;
        core::ptr::write_volatile(heap, 0xa5);
    }

    DESCRIPTOR.run()
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
