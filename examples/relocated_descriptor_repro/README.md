# Relocated Descriptor Reproducer

This example is a minimal Rust reproducer for a relocated descriptor case that
is now expected to work with the current `build_fae_rust` flow.

The scenario is intentionally tiny:

- `DESCRIPTOR` is a global data object placed in `.rom.ram`
- `gen_dyn()` returns `&DESCRIPTOR`
- `use_dyn()` dereferences that descriptor and calls its handler
- `handler()` returns `42`
- `start()` panics if `use_dyn(gen_dyn()) != 42`

When packaged as an FAE, the expected runtime behavior is therefore:

- `DESCRIPTOR` is copied to the relocated writable area
- `gen_dyn()` yields the relocated RAM address of `DESCRIPTOR`
- `use_dyn()` calls the handler through that relocated descriptor
- `start()` returns `42`

Build the plain ELF:

```bash
cargo build --manifest-path examples/relocated_descriptor_repro/Cargo.toml --release
```

Build the Rust/FAE final ELF:

```bash
cargo run --bin build_fae_rust -- \
  --manifest-path examples/relocated_descriptor_repro/Cargo.toml \
  --elf
```

Package it:

```bash
cargo run --bin build_fae -- \
  examples/relocated_descriptor_repro/build/relocated_descriptor_repro.elf
```

What this example exercises:

- `readelf -S` should show a non-empty `.rom.ram`
- `gen_dyn()` must yield the relocated RAM address of `DESCRIPTOR`
- `build_fae_rust` must rewrite executable RAM-data references into the form
  expected by the FAE runtime contract
- `build_fae` must accept the resulting ELF and export its relocation table
