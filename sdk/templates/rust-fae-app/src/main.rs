#![no_std]
#![no_main]

#[macro_use]
extern crate fae_rustrt;

use fae_rustrt::entry;

entry!(fae_app_main);

fn fae_app_main() -> i32 {
    println!("template app");
    0
}
