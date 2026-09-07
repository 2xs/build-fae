# Getting Started

This guide takes you from a fresh checkout to a first `.fae` file and a first `read_fae` inspection.

It contains two practical paths:

- a `C / C++ projects` section for projects that already produce an ELF matching
  the FAE input contract directly
- a `Rust projects` section for Rust applications that use the repository's
  Rust-specific FAE packaging flow

For the full builder contract, see [`build_fae.md`](build_fae.md).

For the binary file format itself, see [`fae-format.md`](fae-format.md).

For user-oriented operational guidance, see [`user-manual.md`](user-manual.md).

For the execution layering (startup code / `start` / `main`-style entrypoints), see [`developer-guide.md`](developer-guide.md).

## What You Will Build

You will use:

- `read_fae`, an FAE file inspector
- `build_fae`, an FAE file builder from ARM ELF input
- a compact assembly Thumb startup under [`../rt0/arm-thumb/rt0-thumb.s`](../rt0/arm-thumb/rt0-thumb.s)
- a compact assembly ARM-state startup under [`../rt0/arm-arm/rt0-arm.s`](../rt0/arm-arm/rt0-arm.s)

The fastest reproducible path is to use the C minimal example under [`../examples/minimal_c`](../examples/minimal_c).

## Prerequisites

You need these tools available in `PATH`:

- `cargo`
- `make`
- `arm-none-eabi-gcc`
- `arm-none-eabi-ld`

Optional extras:

- `qemu-system-arm` for firmware runs under QEMU
- `arm-none-eabi-objdump` for `read_fae` disassembly modes
- `arm-none-eabi-objcopy` for some Rust CRT0 workflows

`build_fae` rebuilds its embedded startup ELFs when the tool itself is compiled.

## C / C++ Projects

### First End-To-End Run

From the repository root:

```bash
make -C examples/minimal_c
cargo run --bin build_fae -- examples/minimal_c/build/minimal.elf
cargo run --bin read_fae -- examples/minimal_c/build/minimal.fae
```

This will:

1. build a minimal ARM ELF
2. select the bundled startup runtime automatically
3. wrap the ELF into a `.fae` file
4. print the structural information stored in the generated FAE

For a firmware-style FAE on QEMU, the shortest path is:

```bash
make -C sdk/minimal-c
cargo run --bin build_fae -- --firmware mps2-an385 sdk/minimal-c/build/minimal.elf
qemu-system-arm -machine mps2-an385 -nographic -semihosting-config enable=on,target=native -kernel sdk/minimal-c/build/minimal.fae
```

Other embedded Cortex-M firmware profiles are also available through `--firmware`:

- `mps2-an386`
- `olimex-stm32-h405`
- `b-l475e-iot01a`

Generated files:

- `examples/minimal_c/build/minimal.elf`
- `examples/minimal_c/build/minimal.fae`
- `examples/minimal_c/build/minimal.gdbinit`

### What To Expect From `read_fae`

`read_fae` should report at least:

- the binary size
- the magic number and version
- the startup code size
- the relocation table state
- the partition offset
- the `.rom.ram`, `.rom`, `.got`, and total writable RAM (`.data + .bss`) sizes
- `Integrity check : OK`

If the file is malformed, `read_fae` stops with an explicit structural error.

### Your Own ELF Input

To replace the minimal example with your own ELF, follow the contract described in [`build_fae.md`](build_fae.md).

For the conceptual model behind `start` versus user-level `main`, see [`developer-guide.md`](developer-guide.md).

At minimum, your ELF must export these symbols:

- `start`
- `__rom_size`
- `__got_size`
- `__rom_ram_size`
- `__ram_size`

The expected section layout is:

- `.rom`
- `.got`
- `.rom.ram`
- `.ram`

In this model:

- `.rom.ram` is the initialized writable data image that startup copies from ROM to the beginning of the RAM data area
- `.ram` is the total writable RAM data area required at runtime, so it covers that copied data plus the trailing zeroed region, typically `.bss`

The current relocation support is intentionally narrow:

- optional `.rel.rom.ram`
- only `R_ARM_ABS32`

Runtime relocation constraints also require that relocation targets correspond to writable data (`.rom.ram` / `.ram`), not `.rom`/`.got`.

### Required Compilation Model (Double Relocatability)

Your ELF must be compiled so that:

- code and read-only accesses are valid under flash placement (PC-relative model)
- writable data and GOT accesses are valid under RAM placement relative to `r9`

### GCC baseline options

Use the same family as XiPFS examples:

```bash
-mthumb -mcpu=cortex-m4 \
-fPIC -ffreestanding \
-msingle-pic-base -mpic-register=r9 \
-mno-pic-data-is-text-relative \
-Wl,-q
```

### Rust baseline approach

- target `thumbv7em-none-eabi` (or another ARM Thumb target that matches your MCU)
- use a linker script that defines `.rom`, `.got`, `.rom.ram`, `.ram`
- export the required symbols (`start`, `__rom_size`, `__got_size`, `__rom_ram_size`, `__ram_size`)

### Debugging With GDB

`build_fae` also writes a matching `.gdbinit` file next to the input ELF.

This file:

- loads symbols for the startup code
- loads symbols for the application ELF
- computes the expected runtime addresses for `.rom`, `.got`, `.rom.ram`, and `.ram`

Before using it, you still need to set the correct target-specific base addresses.

## Rust Projects

The typical starting point is the Rust template under
[`../sdk/templates/rust-fae-app`](../sdk/templates/rust-fae-app).

That template shows the intended split:

- the application crate remains a normal `no_std` Rust binary
- the runtime support stays under [`../sdk/fae-rustrt`](../sdk/fae-rustrt)
- the Rust-specific packaging flow is driven by
  [`build_fae_rust`](build_fae_rust.md)

### Build The Host Tools

From the repository root, build the host-side tools once:

```bash
cargo build --bin build_fae
cargo build --bin read_fae
cargo build --bin build_fae_rust
```

### Build A Rust ELF Compatible With FAE

From the repository root, ask the Rust helper to build the final rewritten ELF:

```bash
cargo run --bin build_fae_rust -- \
  --manifest-path sdk/templates/rust-fae-app/Cargo.toml \
  --elf
```

This produces a Rust ELF that matches the current FAE-oriented section and
relocation contract.

In particular, the current Rust flow handles the relocation patterns exercised
by the repository repros, including:

- executable references from `.rom` code into writable runtime sections
- Thumb high-register SB-relative rewrites
- executable `BREL` relocations that are already SB-relative

### Build A `.fae`

From the repository root:

```bash
cargo run --bin build_fae_rust -- \
  --manifest-path sdk/templates/rust-fae-app/Cargo.toml \
  --fae
```

### Inspect The Result

Once the `.fae` has been produced, inspect it with:

```bash
cargo run --bin read_fae -- path/to/application.fae
```

If you are using the template as-is, the template's own wrapper commands are:

```bash
cargo run --manifest-path sdk/templates/rust-fae-app/xtask/Cargo.toml -- elf
cargo run --manifest-path sdk/templates/rust-fae-app/xtask/Cargo.toml -- fae
```

### Why Rust Uses A Different Flow

For C and C++ examples in this repository, the example build already produces an
ELF in the shape expected by `build_fae`. In that case, `build_fae` can package
the ELF directly.

For Rust, a plain Cargo output is not generally ready to be passed directly to
`build_fae`. The current Rust flow needs an extra orchestration step to:

- discover the relevant Rust-generated sections
- generate the FAE-oriented linker layout
- relink the application into an ELF that matches the current FAE contract
- optionally invoke `build_fae` afterward

That orchestration is the role of the `build_fae_rust` tool documented under
[`build_fae_rust.md`](build_fae_rust.md).

This guide only introduces the minimal commands. For the rationale and current
tool behavior details, see:

- [`build_fae_rust.md`](build_fae_rust.md)
- [`fae-rustrt.md`](fae-rustrt.md)

## Useful Commands

Show help:

```bash
cargo run --bin build_fae -- --help
cargo run --bin read_fae -- --help
```

Inspect the current alignment-related builder options:

```bash
cargo run --bin build_fae -- --help
```

The builder now supports:

- `--align_payload <bytes>` to align the payload partition offset
- `--align_size <bytes>` to align the final `.fae` file size

Run the tests:

```bash
cargo test --offline
```

Build only the reader:

```bash
cargo build --bin read_fae
```

Build only the builder:

```bash
cargo build --bin build_fae
```

Build only the Rust CRT0:

```bash
cargo rustrt0-thumb
```

## Current Limits

The scope is still narrow:

- only the current ARM little-endian flow is supported
- only the current FAE version `0x12` is supported by the reader
- only `.rel.rom.ram` with `R_ARM_ABS32` is supported
- the generated `.gdbinit` still requires manual target address setup
- validation is mostly structural, not full execution on real hardware

## Next Documents

After this guide, the next useful documents are:

- [`build_fae.md`](build_fae.md)
- [`developer-guide.md`](developer-guide.md)
- [`fae-format.md`](fae-format.md)
- [`../examples/minimal_c/README.md`](../examples/minimal_c/README.md)
- [`../examples/minimal_rust/README.md`](../examples/minimal_rust/README.md)
- [`../examples/minimal_startup_asm/README.md`](../examples/minimal_startup_asm/README.md)
- [`../rt0/rustrt0-thumb/README.md`](../rt0/rustrt0-thumb/README.md)
