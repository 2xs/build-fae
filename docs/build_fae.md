# `build_fae` Guide

This guide covers `build_fae` end to end: accepted ELF inputs, external tools, and current limitations.

For the binary file layout itself, see [`fae-format.md`](fae-format.md).
For a practical operator-oriented guide, see [`user-manual.md`](user-manual.md).
For the application-facing execution model (startup code, `start`, `main` layering), see [`developer-guide.md`](developer-guide.md).
For the Rust-specific application flow, see [`build_fae_rust.md`](build_fae_rust.md).

## Purpose

`build_fae` converts an ARM ELF executable into:

- a `.fae` binary container
- a `.gdbinit` helper file for loading debug symbols at the expected runtime addresses

It can also build a startup-only FAE with no application payload.

The tool is part of the Rust rewrite of the historical Python implementation from [`fae_format`](https://gitlab.univ-lille.fr/2xs/pip/fae_format).

`build_fae` packages ELFs that already match the FAE input contract.

In practice:

- C and C++ projects typically use `build_fae` directly once their linker
  script produces the expected ELF layout
- Rust application crates in this repository typically use
  [`build_fae_rust.md`](build_fae_rust.md) first, then rely on `build_fae`
  internally during the final packaging step

## Prerequisites

You need:

- Rust and `cargo`
- the ARM GNU binutils and compiler toolchain

Additional tools are only needed for expert or maintenance flows:

- `arm-none-eabi-objcopy` for the Rust CRT0 helper flows

## Quick Start

From the repository root:

```bash
make -C examples/minimal_c
cargo run --bin build_fae -- examples/minimal_c/build/minimal.elf
cargo run --bin read_fae -- examples/minimal_c/build/minimal.fae
```

This generates:

- `examples/minimal_c/build/minimal.elf`
- `examples/minimal_c/build/minimal.fae`
- `examples/minimal_c/build/minimal.gdbinit`

The C minimal example is documented in [`../examples/minimal_c/README.md`](../examples/minimal_c/README.md).

## Command Line Interface

Basic usage:

```bash
cargo run --bin build_fae -- [options] path/to/application
```

Options:

- `-rt0 VALUE` / `--rt0 VALUE`, where `VALUE` is `thumb`, `thumbv6m`, `arm`, or
  the path to an existing RT0 ELF
- `--allow-rt0-mismatch`, to downgrade an RT0/payload instruction-mode error to
  a warning and produce the FAE anyway
- `--firmware mps2-an385|mps2-an386|olimex-stm32-h405|b-l475e-iot01a`
- `--isa U32` and repeated `--isa-word U32`
- `--abi U32` and repeated `--abi-word U32`
- `-rs` / `--rustlet`
- `-sd` / `--securitydomain`
- `--runtime-entry-point SYMBOL`
- `--startup-alone`
- `--ram-size BYTES`
- `-h`
- `--help`

Behavior:

- the input must be an ELF file
- the output `.fae` is written next to the input ELF
- the output `.gdbinit` companion is written next to the input ELF
- by default, `build_fae` embeds a startup ELF rebuilt while compiling the tool and chosen from the payload entry mode
- Thumb payloads currently use the compact assembly startup from `rt0/arm-thumb/rt0-thumb.s`
- ARM-state payloads currently use the compact assembly startup from `rt0/arm-arm/rt0-arm.s`
- `--firmware` switches to one of the bundled standalone Cortex-M firmware startup profiles
- an ELF path passed to `--rt0` overrides the bundled RT0 selection
- `--startup-alone` is incompatible with an application ELF argument
- `--ram-size` is only valid with `--startup-alone`
- `--runtime-entry-point` defaults to `start`
- explicit descriptors accept decimal or `0x`-prefixed values and their low
  nibble must equal the number of corresponding additional-word options
- an omitted ISA descriptor is inferred from the input ELF and selected RT0
- an explicit ISA descriptor is preserved; a detectable family/subgroup
  mismatch with the ELF is reported as a warning
- the Rustlet and Security Domain switches are convenience aliases for their
  predefined footer fields and default Rustlet RT0, not separate formats

Startup-only usage:

```bash
cargo run --bin build_fae -- --startup-alone --rt0 thumb --ram-size 1024
```

## End-To-End Workflow

Typical workflow:

1. Build an ARM ELF whose layout matches the FAE expectations.
2. Select a startup runtime profile.
3. Run `build_fae` on the ELF.
4. Inspect the generated FAE with `read_fae`.
5. Use the generated `.gdbinit` companion as a starting point for GDB.

In practice:

1. Build the application ELF.
2. Run:

```bash
cargo run --bin build_fae -- path/to/app
```

For a standalone firmware-style Cortex-M FAE on supported boards:

```bash
cargo run --bin build_fae -- --firmware mps2-an385 path/to/app
```

Use `--rt0 path/to/rt0.elf` when you need an explicit startup profile such as
the current `mps2-an521` workaround.

3. Inspect:

```bash
cargo run --bin read_fae -- path/to/app.fae
```

4. Open the generated `.gdbinit`, set the target-specific base addresses, then source it from GDB.

## Supported ELF Contract

`build_fae` does not accept an arbitrary ELF. The input must follow a specific contract.

### Required Symbols

The ELF must export exactly one symbol with each of these names:

- `start`
- `__rom_size`
- `__got_size`
- `__rom_ram_size`
- `__ram_size`

These symbols are read from `.symtab`. Missing symbols or duplicate definitions are rejected.

The `start` symbol here is the current runtime entry point inside the relocated payload, not the low-level FAE entry at file offset `0`. See [`developer-guide.md`](developer-guide.md) for the distinction.

### Startup ELF Contract

When `build_fae` loads low-level startup code, the startup ELF must satisfy this stricter contract:

- it must contain a `._start` section
- it must contain a `_start` symbol
- `_start` must point to the first byte of `._start`
- there must be no relocation sections
- there must be no other allocatable sections

The builder embeds the raw bytes of `._start` at FAE offset `0`.

### Expected Sections

The intended ELF layout contains these sections:

- `.rom`
- `.got`
- `.rom.ram`
- `.ram`

Current behavior:

- section sizes used by `build_fae` come from the exported symbols above, not by recomputing them from section headers
- the partition payload is exported directly from the ELF sections by the current Rust builder
- `read_fae` and the generated `.gdbinit` assume the FAE payload is ordered as `.rom`, then `.got`, then `.rom.ram`
- `.rom.ram` is the initialized writable data image copied by startup into the beginning of RAM
- `.ram` is represented by a size in the footer and denotes the total writable RAM data area required at runtime, so it covers the copied `.rom.ram` bytes plus the trailing zeroed region, typically `.bss`

The C minimal linker script in [`../examples/minimal_c/link.ld`](../examples/minimal_c/link.ld) is the reference example.

### Sections, Not Program Headers

`build_fae` builds the FAE image from ELF sections, not from ELF program
headers (`PT_LOAD` segments).

This is intentional. Program headers are useful to the linker and may be used to
control how relocation models such as ARM RWPI compute their static base, but
they are too coarse to describe the FAE payload contract. In particular, Rust
FAE ELFs may place `.got`, `.rom.ram`, and `.ram` in the same writable
`PT_LOAD` segment while still keeping them as distinct sections.

The section split carries information that the FAE builder and future runtimes
should preserve:

- `.got` is the static-base table
- `.rom.ram` is initialized data copied and relocated at startup
- `.ram` is runtime-only writable memory, typically `.bss` and heap storage

This leaves room for a custom rt0 to protect `.got + .rom.ram` as read-only
after startup relocation while keeping `.ram` writable, when the target MPU can
support such a layout. Building from the merged writable `PT_LOAD` segment would
lose that distinction.

### Relocations

Supported relocation input is intentionally narrow.

Accepted relocation section:

- `.rel.rom.ram`

Accepted relocation type:

- `R_ARM_ABS32`

Current behavior:

- `.rel.rom.ram` is optional
- if absent, the FAE relocation table contains a zero entry count
- `RELA` sections are rejected
- any relocation type other than `R_ARM_ABS32` is rejected
- relocation offsets targeting `.rom`/`.got` (or outside writable ranges) are rejected

For Rust application ELFs produced through [`build_fae_rust.md`](build_fae_rust.md),
additional executable relocation patterns may still appear in `.rel.rom` during
the intermediate build process. `build_fae_rust` is responsible for rewriting or
validating those Rust-specific cases before invoking `build_fae`.

### Architecture Assumptions

Current implementation assumptions:

- 32-bit little-endian ARM ELF
- GNU binutils naming with the `arm-none-eabi-` prefix
- startup entry point exported as `start`

### Double Relocatability Requirement

Input ELF code is expected to be relocatable across two independently placed runtime spaces:

- flash (for `.rom`)
- RAM (for `.got`, `.rom.ram`, `.ram`)

So generated code must follow both constraints:

- code/read-only path must remain valid through PC-relative logic in flash
- writable data path must remain valid through `r9`/GOT-relative logic in RAM

Recommended GCC options (same family as XiPFS examples):

```bash
-mthumb -mcpu=cortex-m4 \
-fPIC -ffreestanding \
-msingle-pic-base -mpic-register=r9 \
-mno-pic-data-is-text-relative \
-Wl,-q
```

For Rust, use an ARM Thumb target and a linker script that explicitly models `.rom`, `.got`, `.rom.ram`, `.ram`, while exporting the required symbols.

## Startup Runtime Inputs

Bundled startup profiles are the default path:

- application ELFs in Thumb mode select the embedded assembly startup derived from `rt0/arm-thumb/rt0-thumb.s`
- application ELFs in ARM state select the embedded assembly startup derived from `rt0/arm-arm/rt0-arm.s`
- `--rt0 thumb|thumbv6m|arm` overrides that auto-selection
- `--firmware mps2-an385|mps2-an386|olimex-stm32-h405|b-l475e-iot01a` selects embedded standalone Cortex-M firmware startup profiles

`-rt0 path/to/rt0.elf` / `--rt0 path/to/rt0.elf` lets you provide an explicit
startup ELF instead of one of the bundled profiles. The builder reads that ELF
and extracts its `._start` section.

## Generated `.gdbinit`

`build_fae` also writes a matching `.gdbinit` file next to the input ELF.

This helper:

- loads symbols for the startup ELF
- loads symbols for the application ELF
- computes runtime addresses for `.rom`, `.got`, `.rom.ram`, and `.ram`
- leaves placeholders for target-specific base addresses such as flash and RAM

Before using it, you still need to edit or define the actual target addresses expected by your board or emulator.

## Current Limitations

Known limitations:

- only the current ARM little-endian flow is supported
- only `.rel.rom.ram` relocations are supported
- only `R_ARM_ABS32` relocations are supported
- the input contract is based on exported symbols and expected section ordering, not on a richer ABI description
- there is no board-specific runner or emulator integration in this repository
- the generated `.gdbinit` still requires manual editing of target base addresses
- validation is focused on file consistency and debug layout, not yet on execution on real hardware

## Troubleshooting

Common failure modes:

- missing `arm-none-eabi-*` tools in `PATH`
- missing required ELF symbols
- duplicate required ELF symbols in `.symtab`
- unsupported relocation sections or relocation types
- an invalid startup ELF passed via `--rt0`

When in doubt:

1. rebuild the ELF from [`../examples/minimal_c`](../examples/minimal_c) or [`../examples/minimal_rust`](../examples/minimal_rust)
2. run `build_fae`
3. inspect the result with `read_fae`
4. if needed, retry with `--rt0 path/to/rt0.elf` to override the bundled startup selection
