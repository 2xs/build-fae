#![no_std]
#![no_main]

use core::panic::PanicInfo;

/*#[unsafe(no_mangle)]
pub static mut COUNTER: i32 = 7;

#[unsafe(no_mangle)]
pub static mut SCRATCH: i32 = 0;
*/
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn start() -> i32 {
    let _mystring = c"I am cstring";
    let mut x = 0;
    x += 1;
    x
}
