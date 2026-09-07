# Bug report

Before commit ec304a63, minimal_rust_print_CStrBuf compiled only on debug profile but worked nonetheless.
Starting from commit ec304a63, both profiles compiled and linked with xipfs-rustrto-thumb (using build_fae_rust and build_fae).
Below is the timeline of the commits and the different commands outputs.

# Original branch :
## Release profile :
Error: unsupported executable relocation pattern for writable runtime symbol .Lanon.a7d138b9e6141af59c2a72e44f4129af.5 in .rel.rom
## Debug profile :
Compilation and runtime work.

# commit 6307cc13:
## Release profile :
Error: unsupported Thumb address materialization for writable runtime symbol .Lanon.a7d138b9e6141af59c2a72e44f4129af.6; expected `add r1, pc` but found 0xf641
## Debug profile :
Compilation and runtime work.

# commit 9559c372:
## Release profile :
Error: unsupported executable relocation pattern for writable runtime symbol .Lanon.b575b88a4d17c77231f178b6c3fd3a41.0 in .rel.rom
## Debug profile :
works

# commit ec304a63:
## Release profile:
Compilation works but runtime does not.
## Debug profile:
Compilation works but runtime does not.

## At runtime:
Hardfaults when trying to print with CStrBuf. slice::raw::from_raw::parts::precondition_check complains about requiring the pointer to be aligned and non-null. There is also an error about the align not being a power of 2.
Data lives at 0x26bdf and pointer seems to be at 0x200061b4

## Commands used to build the dev .fae:
```shell
cargo clean
cargo build
cargo xipfs-rustrt0-thumb dev
cargo run --bin build_fae_rust -- \
            --manifest-path tests/minimal_rust_print_CStrBuf_test/Cargo.toml \
            --elf --profile dev
cargo run --bin build_fae -- --startup-code target/thumbv7em-none-eabi/debug/xipfs-rustrt0-thumb tests/minimal_rust_print_CStrBuf_test/build/minimal_rust_print_CStrBuf.elf
```
