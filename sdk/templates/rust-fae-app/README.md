# Rust FAE App Template

This template shows the intended split:

- the runtime stays in
  [`sdk/fae-rustrt`](../../fae-rustrt)
- the build orchestration lives in
  [`crates/build-fae-rust`](../../../crates/build-fae-rust)
- the application crate remains a normal `no_std` Rust binary

The underlying helper binary is `build_fae_rust`.

## Commands

Build the final ELF:

```bash
cargo run --manifest-path sdk/templates/rust-fae-app/xtask/Cargo.toml -- elf
```

Build a `.fae` with the default startup:

```bash
cargo run --manifest-path sdk/templates/rust-fae-app/xtask/Cargo.toml -- fae
```

Build a bootable image:

```bash
cargo run --manifest-path sdk/templates/rust-fae-app/xtask/Cargo.toml -- bootable mps2-an385
```
