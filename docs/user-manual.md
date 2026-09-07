# FAE User Manual

This manual explains how to produce and inspect XiPFS-compatible FAE files, and which constraints apply to input ELF binaries.

For the binary on-disk format, see [`fae-format.md`](fae-format.md).

For the execution layering (`low_level_entry_point`, `runtime_entry_point`, `high_level_entry_point`), see [`developer-guide.md`](developer-guide.md).

For the runtime syscall families exposed to programs, see [`fae-runtime-environment.md`](fae-runtime-environment.md).

## Execution Model Summary

FAE assumes two independently placed runtime spaces:

- flash/NVM for `.rom` (code + read-only data)
- RAM for `.got`, `.rom.ram`, and `.ram`

For writable data, the intended runtime layout is:

- startup copies `.rom.ram` from ROM into the beginning of the RAM data area
- the remaining writable RAM up to `ram_size` follows it and is typically used as `.bss`

So `ram_size` represents the total writable RAM data requirement: initialized data plus trailing zero-initialized data.

An input ELF must therefore be **doubly relocatable**:

- code and read-only references must be valid with PC-relative execution in flash
- writable data and GOT accesses must be valid relative to `r9` (PIC base register for RAM/GOT model)

## Tooling Flow

1. Build an ARM ELF with PIC/GOT-compatible options.
2. Select a startup profile.
3. Run `build_fae`.
4. Inspect with `read_fae`.

## Typical Commands

From repository root:

```bash
cargo run --bin build_fae -- path/to/app
cargo run --bin read_fae -- path/to/app.fae
```

For firmware-style Cortex-M outputs that should boot directly under QEMU on the supported boards:

```bash
cargo run --bin build_fae -- --firmware mps2-an385 path/to/app
```

Advanced modes:

- `-rt0 path/to/rt0.elf` or `--rt0 path/to/rt0.elf` for an explicit startup ELF
- `--firmware mps2-an385|mps2-an386|olimex-stm32-h405|b-l475e-iot01a` for the bundled standalone firmware profiles

## ELF Compilation Requirements

The ELF must export:

- `start`
- `__rom_size`
- `__got_size`
- `__rom_ram_size`
- `__ram_size`

Expected section model:

- `.rom`
- `.got`
- `.rom.ram`
- `.ram`

Relocation constraints currently enforced by `build_fae`:

- only `.rel.rom.ram`
- only `R_ARM_ABS32`
- relocation targets must be in writable data range (`.rom.ram` or `.ram`), not in `.rom`/`.got`

## GCC Reference Options

Typical options (same family as XiPFS examples):

```bash
-mthumb -mcpu=cortex-m4 \
-fPIC -ffreestanding \
-msingle-pic-base -mpic-register=r9 \
-mno-pic-data-is-text-relative \
-Wl,-q
```

These options are the practical baseline for code that respects the flash/RAM split expected by FAE startup relocation.

## Rust Reference Approach

For Rust applications targeting FAE:

- target ARM Thumb (`thumbv7em-none-eabi` for Cortex-M4 class targets)
- use custom linker script exposing `.rom`, `.got`, `.rom.ram`, `.ram` and required size symbols
- ensure generated code follows PIC/GOT access discipline compatible with the `r9`-based runtime relocation model

## Debugging

`build_fae` generates a matching `.gdbinit` file next to the ELF.  
Set target flash/RAM base addresses before sourcing it in GDB.

## Startup-Only Mode

If you want a FAE containing only low-level startup code and no application payload:

```bash
cargo run --bin build_fae -- --startup-alone --rt0 thumb --ram-size 1024
```

You can use:

- a bundled startup selected with `--rt0 thumb|thumbv6m|arm`
- or an explicit startup ELF selected with `--rt0 path/to/rt0.elf`

When you provide an explicit startup ELF, it must contain:

- one `._start` section
- one `_start` symbol at the first byte of that section
- no relocation sections
- no other allocatable sections

## Related Docs

- [`getting-started.md`](getting-started.md)
- [`developer-guide.md`](developer-guide.md)
- [`fae-runtime-environment.md`](fae-runtime-environment.md)
- [`build_fae.md`](build_fae.md)
- [`fae-format.md`](fae-format.md)
