# Minimal Startup-Only Assembly Example

This example produces a minimal low-level startup ELF compatible with `build_fae --startup-code`.

It is intentionally low level:

- a single `._start` section
- a single `_start` symbol at the first byte of that section
- no application payload

## Build the Startup ELF

From the repository root:

```bash
make -C examples/minimal_startup_asm
```

This generates:

- `examples/minimal_startup_asm/build/startup.elf`

## Build a Startup-Only FAE

From the repository root:

```bash
cargo run --bin build_fae -- --startup-code examples/minimal_startup_asm/build/startup.elf --startup-alone --ram-size 1024
```

This generates:

- `examples/minimal_startup_asm/build/startup.fae`
- `examples/minimal_startup_asm/build/startup.gdbinit`
