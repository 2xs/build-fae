# Compilation Tooling

This document lists the host tools used across this repository and separates
what is required to maintain the repository and its versioned artifacts from
what is only used by non-core or experimental workflows.

## Scope

There are three distinct build layers in this repository:

1. the Rust host tools (`build_fae`, `read_fae`, `xtask`)
2. the ARM ELF inputs built by examples, starters, or external applications
3. startup ELFs rebuilt automatically while compiling `build_fae`

Because `build_fae` now rebuilds its embedded startup ELFs during its own Cargo
build, a host that compiles `build_fae` must also provide the ARM assembler and
linker toolchain used for those startup inputs.

## Required Tools

These tools are required to cover the maintained compilation surface of this
repository, including regeneration of versioned startup binaries and the
starter code kept in-tree:

| Tool | Why it is required |
| --- | --- |
| `cargo` and the Rust toolchain | Builds `build_fae`, `read_fae`, and `xtask`. |
| `make` | Drives the maintained Makefile-based example and starter builds. |
| `arm-none-eabi-gcc` | Compiles the C and assembly inputs used by `examples/minimal_c`, `examples/minimal_startup_asm`, `sdk/minimal-c`, and the startup ELFs rebuilt while compiling `build_fae`. |
| `arm-none-eabi-ld` | Links the ARM startup ELFs rebuilt while compiling `build_fae` and is used as the linker in the `xtask`-driven Rust CRT0 build flow. |
| `arm-none-eabi-objcopy` | Required by `cargo rustrt0-thumb`, `cargo rustrt0-arm`, and `xtask` to export the generated Rust CRT0 binaries. |
| `arm-none-eabi-objdump` | Used by `read_fae` disassembly workflows and part of the documented ARM binutils toolset expected around this project. |
| Rust ARM targets: `thumbv7em-none-eabi` and `armv7a-none-eabi` | Needed when building the in-tree Rust CRT0 binaries (`cargo rustrt0-thumb`, `cargo rustrt0-arm`) and Rust ARM examples. |
| `meson` | Required by the maintained `sdk/faeulibc` build. |
| `ninja` | Required by the maintained `sdk/faeulibc` build. |
| `arm-none-eabi-ar` | Required by the maintained `sdk/faeulibc` build. |
| `arm-none-eabi-as`, `arm-none-eabi-g++`, `arm-none-eabi-nm`, `arm-none-eabi-strip` | Referenced by the Meson cross file used by `sdk/faeulibc`. |

### Notes On Examples And Starters

The standard examples do not introduce a separate exotic tool family:

- [`../examples/minimal_c`](../examples/minimal_c),
  [`../examples/minimal_startup_asm`](../examples/minimal_startup_asm), and
  [`../sdk/minimal-c`](../sdk/minimal-c) stay within the ARM GNU
  toolchain plus `make`
- [`../examples/minimal_rust`](../examples/minimal_rust) stays within the Rust
  toolchain plus the ARM Rust target
- [`../sdk/faeulibc`](../sdk/faeulibc) is the only maintained starter
  that adds Meson/Ninja/Picolibc-specific tooling

### Minimum Core Host-Tool Build

If the goal is to compile the main host-side binaries shipped by this
repository, the minimum requirement is:

- `cargo` and the Rust toolchain
- `arm-none-eabi-gcc`
- `arm-none-eabi-ld`

That is enough for:

```bash
cargo build --bin build_fae
cargo build --bin read_fae
```

## Optional Tools

These tools are not required for the maintained compilation surface above.
They belong to execution or validation side paths.

| Tool | Used for |
| --- | --- |
| `qemu-system-arm` | Boot and smoke-test workflows for `sdk/minimal-c` and the Cortex-M firmware profiles. |

## Notes

- The current `build_fae` default flow rebuilds its embedded startup ELFs at
  Cargo build time.
- If the Rust ARM targets are not already installed, they can be added with
  `rustup target add thumbv7em-none-eabi armv7a-none-eabi`.

The host-side workspace and the embedded projects are checked separately:

```sh
cargo test --workspace
cargo embedded-check
```

The first command tests the portable tooling on the host. The second cross-checks
the ARM RT0s, examples, and reproduction projects with their intended targets.
