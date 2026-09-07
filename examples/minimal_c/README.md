# Minimal C Example

This example produces a small ARM ELF compatible with `build_fae`.

It is intentionally minimal:

- no dependency on the historical Python tooling
- no dependency on the old `stdriot` support code
- a linker script that exports the section size symbols expected by `build_fae`

## Build the ELF

From the repository root:

```bash
make -C examples/minimal_c
```

This generates:

- `examples/minimal_c/build/minimal.elf`

## Build the FAE

From the repository root:

```bash
cargo run --bin build_fae -- examples/minimal_c/build/minimal.elf
```

This generates:

- `examples/minimal_c/build/minimal.fae`
- `examples/minimal_c/build/minimal.gdbinit`
