Flow of events:

```shell
cargo clean
cargo build
cargo xipfs-rustrt0-thumb
cargo run --bin build_fae_rust -- \
      --manifest-path tests/minimal_rust_print_test/Cargo.toml \
      --elf
cargo run --bin build_fae -- --startup-code target/thumbv7em-none-eabi/release/xipfs-rustrt0-thumb tests/minimal_rust_print_test/build/minimal_rust_print_test.elf
```

## Status

This case is now expected to work.

The compiler emits `R_ARM_THM_MOVW_BREL_NC` (87) / `R_ARM_THM_MOVT_BREL` (88) for the
`.ram` statics (`FORMER_GOT` etc.). These relocations are already SB-relative, so
`build_fae` now accepts them in executable sections.

To inspect the relocations involved:
```shell
arm-none-eabi-readelf -rW tests/minimal_rust_print_test/build/minimal_rust_print_test.elf | rustfilt | less
```
