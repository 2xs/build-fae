# FAE Developer Guide

This guide explains the execution model expected by the current FAE tooling from an application developer point of view.

It is intentionally different from:

- [`getting-started.md`](getting-started.md), which focuses on the fastest first run
- [`build_fae.md`](build_fae.md), which focuses on the builder contract
- [`fae-format.md`](fae-format.md), which describes the on-disk layout
- [`fae-runtime-environment.md`](fae-runtime-environment.md), which describes the runtime syscall ABI exposed to programs

## Mini Glossary

Some recurring words in this repository refer to different layers.

- `rt0`:
  the low-level startup code that begins at byte offset `0` in the final `.fae`
- `startup`:
  a generic term for that low-level `rt0` layer; in CLI/docs this often means the startup ELF chosen by `build_fae`
- `firmware`:
  a startup profile that is directly bootable on a board, typically with a vector table and reset wrapper
- `runtime`:
  the language-facing support layer that runs after relocation, usually around the exported payload symbol `start`
- `sdk`:
  the developer-facing starters, templates, and helper runtime layers used to build payload ELFs

In short:

- `rt0` / `startup` = lowest layer
- `runtime` = language bridge above relocation
- `sdk` = reusable developer-facing packaging of that bridge and example app structure

## Three Entry Layers

For application development, it is useful to distinguish three different entry layers.

### 1. Low-Level Entry Point

This is the code located at byte offset `0` in the final `.fae` file.

In the current repository, this code is the embedded CRT0 image:

- historical C CRT0: `_start`
- current Rust CRT0: `_start`

This is the first instruction stream executed by an FAE loader.

Responsibilities of the low-level entry point:

- validate the FAE footer and metadata
- compute the partition layout
- relocate `.got`, `.rom.ram`, and `.ram`
- initialize the execution context expected by the application runtime
- call the application entry offset stored in the FAE footer after relocation

So, when we say "executing an FAE file", execution really begins here, not in the user-facing `main`.

### Stack And Interrupt Assumptions

The current low-level startup code assumes that a valid execution stack already exists before it begins relocation work.

For a hosted/XIP-style FAE:

- the loader or host runtime is expected to provide a writable stack in RAM before branching to FAE offset `0`
- the startup code may temporarily reuse `sp` as a cursor while parsing file metadata
- this is acceptable only because the host-controlled execution environment is expected to make that safe for the duration of startup

For a firmware-style FAE on Cortex-M:

- the FAE itself owns reset handling and installs `MSP` from its vector table
- exceptions and interrupts also use `MSP`, so pointing `sp` at flash during early startup is unsafe
- the bundled Cortex-M firmware startup therefore masks interrupts in `Reset_Handler` before entering the shared relocation core
- interrupts are expected to remain masked until the runtime entry point (`start`) explicitly enables them again if needed

In short:

- hosted/XIP FAE: the external launcher owns stack safety assumptions
- firmware FAE: the bundled reset wrapper owns stack safety and masks interrupts during relocation

### 2. Runtime Entry Point

This is the first entry point inside the relocated application payload.

In the historical XiPFS flow, that symbol is `start`.

`build_fae` stores the offset of this symbol in the FAE footer, and CRT0 branches to it after relocation. In the current Rust implementation, the builder still expects the same exported symbol:

- `start`

This runtime entry point is where language-specific runtime setup can happen.

Typical responsibilities:

- initialize runtime-global pointers
- expose the relocated GOT or context to higher layers
- perform runtime setup before user code
- call the high-level application entry point

In the old C-based stack, this role was provided by `stdriot.c`, whose `start(crt0_ctx_t *ctx)` eventually called `main(argc, argv)`.

### 3. High-Level Entry Point

This is the entry point that application developers usually think of as "the program entry point".

Examples:

- C: `main`
- higher-level Rust model: a user-facing `main`-like function

This is not the first instruction executed from the FAE file, and it is not the symbol currently stored in the FAE footer.

It is a language-level convention sitting above the runtime entry point.

## Historical Three-Stage Flow

The historical C/XiPFS model was effectively:

1. CRT0 `_start`
2. runtime `start`
3. application `main`

In other words:

- CRT0 was shipped by the FAE tooling
- `start` was shipped by the runtime support layer (`stdriot`)
- `main` was supplied by the application developer

This explains why developers could feel that "the entry point is main", while the converter itself still encoded `start` as the effective FAE entrypoint.

## Current Repository Contract

The current repository implements only part of that historical layering.

What is present today:

- low-level startup runtimes in [`../rt0/arm-thumb`](../rt0/arm-thumb), [`../rt0/arm-arm`](../rt0/arm-arm), [`../rt0/rustrt0-thumb`](../rt0/rustrt0-thumb), and [`../rt0/rustrt0-arm`](../rt0/rustrt0-arm)
- a builder that expects an ELF exporting `start`

What is not yet provided as a reusable layer in this repository:

- a generic C runtime layer that exposes `start` and then calls `main`
- a generic Rust runtime layer that exposes `start` and then calls a user-level `main`

So today, if you want to build an application ELF for this repository, your application must normally export `start` directly.

That is why the minimal examples are currently:

- [`../examples/minimal_c`](../examples/minimal_c): exports `start` directly
- [`../examples/minimal_rust`](../examples/minimal_rust): exports `start` directly

## About an Ultra-Minimal FAE

Conceptually, one can imagine an ultra-minimal FAE where:

- the code at offset `0` is itself the full application entrypoint
- there is no separate CRT0/runtime layer
- `.got`, `.rom.ram`, and `.ram` may all be empty
- relocation support is entirely the responsibility of the code author

That model is meaningful as a conceptual lower bound.

However, it is not the mode currently produced by `build_fae` in this repository. The current builder always assembles an FAE around an explicit CRT0 + metadata + partition payload model.

So the "three layers" are a good conceptual model, but the current implementation only supports the variant where layer 1 exists and layer 2 is encoded as `start`.

The builder now exposes that layer-1 choice explicitly through:

- `--rt0 path/to/rt0.elf` (or `-rt0`), to select a custom low-level startup ELF
- `--startup-alone`, to build a FAE containing only layer 1

## Language Mapping

### C

There are two reasonable models:

- current repository model: developer provides `start`
- historical stdriot-style model: runtime provides `start`, developer provides `main`

The second model is historically important, but it is not yet packaged here as a reusable layer.

### Rust

Likewise, there are two different concerns:

- runtime entry point exported to FAE tooling: currently `start`
- higher-level user function: potentially `main`, but only if a runtime layer bridges to it

In standard hosted Rust, `main` is not the true first machine-level entrypoint either. A language/runtime bridge sits below it. For FAE, a similar layering makes sense, but this repository does not yet ship a generic Rust runtime layer of that form.

## Recommended Terminology

To avoid ambiguity, prefer these names in the documentation:

- `low_level_entry_point`: the code at FAE offset `0`
- `runtime_entry_point`: the relocated payload entry currently exported as `start`
- `high_level_entry_point`: the language-level user entry such as `main`

This vocabulary is more precise than calling all three layers "the entrypoint".

## What Developers Must Provide Today

For the current Rust builder, an input ELF must provide:

- `start`
- `__rom_size`
- `__got_size`
- `__rom_ram_size`
- `__ram_size`

It must also follow the expected section model:

- `.rom`
- `.got`
- `.rom.ram`
- `.ram`

For the exact builder contract, see [`build_fae.md`](build_fae.md).

## Future Directions

A more complete developer-facing stack could add one or both of these:

- a reusable C runtime layer providing `start -> main`
- a reusable Rust runtime layer providing `start -> user main`

If that happens, the builder contract could remain centered on `start`, while application developers mostly interact with `main`.

For the runtime services that such layers are expected to consume, see [`fae-runtime-environment.md`](fae-runtime-environment.md).
