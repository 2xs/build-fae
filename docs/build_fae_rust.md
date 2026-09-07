# `build_fae_rust` Guide

This guide covers the Rust-specific application packaging flow for FAE.

For the binary file layout itself, see [`fae-format.md`](fae-format.md).
For the lower-level `build_fae` ELF contract, see [`build_fae.md`](build_fae.md).
For the current Rust runtime investigation notes, see [`fae-rustrt.md`](fae-rustrt.md).

## Purpose

`build_fae_rust` orchestrates the current Rust application flow for FAE:

1. discovery link with `rust-lld`
2. collect `R_ARM_ABS32` target sections
3. generate a throwaway FAE linker script
4. final link with that script
5. optional `build_fae` packaging

The runtime itself remains in [`../sdk/fae-rustrt`](../sdk/fae-rustrt).

The generated Rust linker script may use ELF program headers to force `.got`,
`.rom.ram`, and `.ram` into one writable `PT_LOAD` segment. This keeps ARM RWPI
static-base calculations coherent with the `r9` value installed by startup.
That segment grouping is a linker constraint only: `build_fae` still packages
the final ELF from the individual `.rom`, `.got`, `.rom.ram`, and `.ram`
sections. See [`build_fae.md`](build_fae.md#sections-not-program-headers).

In the current repository state, `build_fae_rust` also handles the Rust-specific
executable relocation cases exercised by the repro examples:

- executable references from `.rom` code into writable runtime data
- Thumb high-register SB-relative rewrites
- optimized Thumb address materializations where LLVM interleaves multiple
  `MOVW`/`MOVT` pairs before their matching `ADD rN, pc` instructions
- executable `BREL` relocations that are already SB-relative

## When To Use It

Use `build_fae_rust` for Rust application crates in this repository.

Use `build_fae` directly only when you already have an ELF that matches the FAE
input contract documented in [`build_fae.md`](build_fae.md).

In practice:

- C and C++ projects typically reach that contract directly through an adapted
  linker script
- Rust projects in this repository typically go through `build_fae_rust` first
- the repro-style examples under `examples/relocated_*` are useful references for
  the Rust relocation cases that this tool now supports

## Usage

Choose the packaging mode explicitly:

- `--elf` to stop after producing the rewritten ELF
- `--fae` to also package a `.fae` with the default runtime startup
- `bootable <board>` to package a board-specific bootable FAE

You can also pass:

- `--align_payload <bytes>` to align the payload partition offset
- `--align_size <bytes>` to align the final `.fae` file size
- `--bin <name>` to override the produced executable name
- `--target <triple>` to override the default target
- `-rt0` / `--rt0` followed by `thumb`, `thumbv6m`, `arm`, or an RT0 ELF path;
  the names select an embedded RT0 and an existing path selects that exact RT0
- `--allow-rt0-mismatch` to downgrade an RT0/payload instruction-mode error to
  a warning and deliberately produce the FAE anyway
- `--isa <u32>` followed by repeated `--isa-word <u32>` options to write an
  explicit FAE 1.0 ISA descriptor and its additional words
- `--abi <u32>` followed by repeated `--abi-word <u32>` options to write an
  explicit FAE 1.0 ABI descriptor and its additional words
- `-rs` / `--rustlet` and `-sd` / `--securitydomain` as convenience profiles
  for the Oxide SE application and Security Domain ABI respectively

The low nibble of each explicit descriptor is the number of following words;
the command rejects a different number of `--isa-word` or `--abi-word`
arguments. Without `--isa`, the ISA is inferred from the converted ELF and the
selected RT0. With `--isa`, the requested value is written verbatim and a
warning reports any family/subgroup disagreement that the ELF makes visible.

The Rustlet aliases feed the same footer builder as the explicit options. They
also select the matching Rustlet RT0 by default; an explicit `--rt0` remains
authoritative whether its argument names an embedded RT0 or an ELF file.

Build only the ELF:

```bash
cargo run --bin build_fae_rust -- \
  --manifest-path sdk/templates/rust-fae-app/Cargo.toml \
  --elf
```

Build a `.fae` with the default runtime startup:

```bash
cargo run --bin build_fae_rust -- \
  --manifest-path sdk/templates/rust-fae-app/Cargo.toml \
  --fae
```

Build a bootable FAE:

```bash
cargo run --bin build_fae_rust -- \
  --manifest-path sdk/templates/rust-fae-app/Cargo.toml \
  bootable mps2-an385
```

Build an aligned `.fae`:

```bash
cargo run --bin build_fae_rust -- \
  --manifest-path sdk/templates/rust-fae-app/Cargo.toml \
  --align_payload 32 \
  --align_size 32 \
  --fae
```

Build a Rustlet Security Domain with its standard compact ABI profile:

```bash
cargo run --bin build_fae_rust -- \
  --manifest-path path/to/security-domain/Cargo.toml \
  --securitydomain \
  --fae
```
