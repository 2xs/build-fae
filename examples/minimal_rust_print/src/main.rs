#![no_std]
#![no_main]

use core::panic::PanicInfo;

use rust_xipfs_lib::{
    print,
    stdriot::{get_file_size, get_led, get_temp, isprint, set_led, strtol},
};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn main() -> i32 {
    let temp = get_temp();
    print!("Test if unicode works è è è @@@@\n");
    print!("temp is {}\n", temp);
    let my_char: char = 'c';
    let res = isprint(my_char as u8);
    if res > 0 {
        print!("{} is a printable character\n", my_char);
    } else {
        print!("This is not a printable character.\n");
    }
    let base = 10;
    let string = "526";
    let conv_nb = strtol(string, base);
    if conv_nb == 526 {
        print!("Converted number is {}\n", conv_nb);
    }
    let led_status = get_led(0);
    set_led(0, led_status ^ 1);
    let mut size: usize = 0;
    let status = get_file_size("/nvme0p0/minimal_rust_print_test.fae", &mut size);
    if status < 0 {
        print!("Get file size did not work.\n Size found : {}\n", size);
    } else {
        print!("Size of file: {} bytes\n", size);
    }
    0
}
