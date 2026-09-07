#![no_std]
#![no_main]

extern crate alloc;
#[macro_use]
extern crate fae_rustrt;

use alloc::boxed::Box;
use fae_rustrt::{entry, read_char};

entry!(fae_app_main);

fn fae_app_main() -> i32 {
    let value = Box::new(41_i32);
    println!("Hello World! value={}", *value);
    eprintln!("Hello Error! value={}", *value);
    println!("Type one character:");
    let ch = read_char();
    println!("Read back: {}", ch as char);
    *value + 1
}
