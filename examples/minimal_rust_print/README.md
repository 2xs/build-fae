# minimal_rust_print

Minimal Rust payload using `rust-xipfs-lib` (`printf`, `XipfsRt0CtxData`).
Demonstrates the `build_fae_rust` build path for workspace crates that depend on
`rust-xipfs-lib`.

## Build

```bash
# 1. Build the payload ELF (nightly, ropi-rwpi, two-pass with auto linker script)
cargo run --bin build_fae_rust -- \
  --manifest-path examples/minimal_rust_print/Cargo.toml \
  --elf

# 2. Build the xipfs startup
cargo xipfs-rustrt0-thumb

# 3. Package into a .fae
cargo run --bin build_fae -- \
  --startup-code target/thumbv7em-none-eabi/debug/xipfs-rustrt0-thumb \
  --runtime-entry-point start \
  examples/minimal_rust_print/build/minimal_rust_print.elf
```

Output: `examples/minimal_rust_print/build/minimal_rust_print.fae`

## Notes

This example relies on two Rust-specific behaviors that are now handled by the toolchain:

- Thumb high-register address materialization in the SB-relative rewrite path
- executable `BREL` relocations, which are already SB-relative and should be accepted
