# Tooling Crates

`crates/` contains the maintained host-side Rust tooling.

This layer is split into:

- end-user CLI crates
- internal library crates shared by those CLIs
- one compatibility crate kept for shared tests and helper reuse

## End-User CLIs

- [`build-fae/`](build-fae): `build_fae`, which packages an application ELF into a `.fae`
- [`read-fae/`](read-fae): `read_fae`, which inspects and validates a `.fae`
- [`check-elf-rel/`](check-elf-rel): helper CLI to inspect relocation-targeted ELF sections
- [`generate-fae-rust-ld/`](generate-fae-rust-ld): helper CLI to derive a Rust FAE linker script from a discovery ELF

## Internal Libraries

- [`fae-core/`](fae-core): shared format definitions, constants, and pure helpers
- [`fae-elf/`](fae-elf): ELF parsing, symbol extraction, relocation handling, and Rust section-plan discovery
- [`fae-build/`](fae-build): `build_fae` logic
- [`fae-read/`](fae-read): `read_fae` logic
- [`build-gdbinit/`](build-gdbinit): companion `.gdbinit` generation

## Compatibility Crate

- [`xiprfs-fae-tools/`](xiprfs-fae-tools): small compatibility layer kept for shared tests and transitional reuse inside the workspace
