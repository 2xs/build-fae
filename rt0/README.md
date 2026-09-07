# RT0 Layer

`rt0/` contains the in-repository startup runtimes and firmware startup
profiles used by the FAE tooling.

This layer is versioned together with the tooling because the builder, the
reader, the embedded startup assets, and the startup code itself evolve
together during development.

It is not the same thing as the user-facing CLI tools:

- the CLI tools live under [`../crates/`](../crates)
- the runtime layer lives under [`./`](.)

## Contents

- [`arm-thumb/`](arm-thumb): shared Thumb assembly relocation core
- [`arm-arm/`](arm-arm): shared ARM-state assembly relocation core
- [`rustrt0-thumb/`](rustrt0-thumb): Rust CRT0 crate for ARM Thumb payloads
- [`rustrt0-arm/`](rustrt0-arm): Rust CRT0 crate for ARM-state payloads
- [`bootable/`](bootable): firmware-style Cortex-M startup profiles
- [`embedded/`](embedded): documentation anchor for the startup ELF flow used
  by `build_fae`

## Calling Convention

The normative FAE startup ABI is the minimal register convention implemented by
the shared assembly relocation cores at
[`arm-thumb/rt0-thumb.s`](arm-thumb/rt0-thumb.s) and
[`arm-arm/rt0-arm.s`](arm-arm/rt0-arm.s):

- `pc`: entry at `_start` inside the startup image
- `sp`: valid caller stack
- `lr`: return address for the caller
- `r0-r3`: forwarded unchanged to the relocated payload entrypoint on the
  success path
- `r9`: start of the RAM window granted to the payload, and later the relocated
  static base / GOT base

Everything else needed by the relocation logic is discovered from the startup /
FAE image itself: binary size, relocation count/table, footer sizes,
entrypoint offset, and magic/version.

The caller is responsible for RAM sizing:

- it must inspect the footer first
- it must grant enough RAM starting at `r9` for `.got + .rom.ram + .ram`
- otherwise relocation overflows the granted RAM region

The minimal relocation core reports startup rejection by returning to the
caller with `r0 != 0` before the payload entrypoint is called.

Current shared-core codes are:

- `1`: invalid file version
- `3`: out-of-bounds offset
- `4`: cannot relocate offsets in `.rom`
- `5`: cannot relocate offsets in `.got`

Code `2` is a legacy reserved slot and is not emitted by the current minimal
ABI core.

If the payload entrypoint returns, the core resumes at the original caller
return address with that caller return preserved across the internal `blx`.

Bootable Cortex-M wrappers may layer additional board-local fatal categories
on top of that contract. The generic wrapper currently reports:

- `6`: hard fault
- `7`: memmanage fault

On the success path, the current generic bootable Cortex-M wrapper then exits
through semihosting `SYS_EXIT`.

The larger C and Rust startups in this directory are intended as more readable
reference implementations of the same startup behavior. They should preserve
the same observable ABI: enter with `pc/sp/lr/r0-r3/r9`, relocate the payload
view, then call the payload entrypoint themselves with `r0-r3` preserved and
`r9` set to the relocated static base / GOT base.

## Build

The canonical developer entry points are the workspace cargo aliases:

```bash
cargo rustrt0-thumb
cargo rustrt0-arm
```

These aliases forward to `xtask`, which builds the runtime ELF and exports the
corresponding raw binary under `build/`.

Equivalent explicit form:

```bash
cargo run -p xtask -- rustrt0-thumb
cargo run -p xtask -- rustrt0-arm
```

## Embedded Startup Assets

`build_fae` rebuilds its embedded startup ELFs into Cargo `OUT_DIR` while the
tool itself is compiled.

The helper script below is still useful when you want ad hoc startup ELFs
under `build/embedded-startups` while working on startup code or bundled
firmware profiles:

```bash
./scripts/regenerate_embedded_startups.sh
```

## Role In The Workspace

The `rustrt0-*` crates are workspace runtime crates used for development and
maintenance of startup code. They are intentionally kept separate from the
user-facing CLI crates.
