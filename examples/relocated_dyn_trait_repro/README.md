# Relocated `dyn Trait` Reproducer

This example is a minimal Rust reproducer for a trait-object case that looks
very similar to the constant descriptor case, but is now expected to work with
the current `build_fae_rust` flow.

The scenario is intentionally small:

- `HANDLER_IMPL` is a global object
- `DESCRIPTOR` is a global `&'static dyn Handler`
- `gen_dyn()` returns that trait object
- `use_dyn()` calls `run()` through dynamic dispatch
- `start()` panics if the returned value is not `42`

The important detail is that the trait object is a constant fat pointer:

- one word points to the data object
- one word points to the vtable

When building for FAE, both of those words must remain consistent with the
runtime RAM/flash split.

Build the plain ELF:

```bash
cargo build --manifest-path examples/relocated_dyn_trait_repro/Cargo.toml --release
```

Build the Rust/FAE final ELF:

```bash
cargo run --bin build_fae_rust -- \
  --manifest-path examples/relocated_dyn_trait_repro/Cargo.toml \
  --elf
```

Package it:

```bash
cargo run --bin build_fae -- \
  examples/relocated_dyn_trait_repro/build/relocated_dyn_trait_repro.elf
```

What this example exercises:

- a relocated trait-object descriptor stored in writable runtime data
- executable references from `.rom` code into `.rom.ram`
- the Rust-side rewrite and validation path used before final packaging
