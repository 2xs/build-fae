# Minimal Rust Example

This example produces a small ARM ELF in Rust compatible with `build_fae`.

It is intentionally minimal:

- `no_std`, `no_main`
- a custom linker script exporting the section size symbols expected by `build_fae`
- no dependency on the historical Python tooling

## Build the ELF

From the repository root:

```bash
make -C examples/minimal_rust
```

This generates:

- `target/thumbv7em-none-eabi/release/minimal_rust`

## Build the FAE

From the repository root:

```bash
cargo run --bin build_fae -- target/thumbv7em-none-eabi/release/minimal_rust
```

This generates:

- `target/thumbv7em-none-eabi/release/minimal_rust.fae`
- `target/thumbv7em-none-eabi/release/minimal_rust.gdbinit`
