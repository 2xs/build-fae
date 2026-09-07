# FAE + Picolibc Starter

C starter for building an FAE binary with:

- a local intact copy of `picolibc/`
- a project-specific `start(context_s *ctx)` entry point
- a dedicated FAE linker script
- `stdin`, `stdout`, and `stderr` retargeted to a local semihosting backend
- no `fopen` or POSIX I/O support in the first iteration

## Layout

```text
sdk/faeulibc/
  app/                  User example
  fae/                  FAE glue layer
  picolibc/             Intact upstream snapshot
  Makefile              Local build orchestration
```

The integration follows Picolibc's "embedded source" approach:

- do not use `-specs=picolibc.specs`
- keep the upstream libc separate from project glue
- configure the library locally to the project
- pin an explicit Picolibc version

## Current State

The starter provides the project layout and glue files, but a full
Picolibc build still requires `meson` in addition to `ninja`.

Required tools:

- `arm-none-eabi-gcc`
- `arm-none-eabi-ar`
- `meson`
- `ninja`
- `cargo` to package a `.fae`

## Build

From the repository root:

```bash
make -C sdk/faeulibc
```

If `sdk/faeulibc/picolibc/` is not already present:

- if a git submodule is declared at that path, the `Makefile` initializes it
- otherwise, it automatically clones Picolibc from
  `https://github.com/picolibc/picolibc.git` at tag `1.8.11`

The `Makefile`:

1. initializes the submodule, or clones Picolibc at a pinned tag
2. configures a local Picolibc build
3. compiles the FAE glue with the same PIC model as the rest of the repository
4. links everything with `fae/link.ld`

## Heap Configuration

Picolibc's malloc implementation still goes through `sbrk`, but this starter
does not provide a custom project `sbrk()`. Instead, the linker script exports
`__heap_start` and `__heap_end`, which are consumed by Picolibc's fallback
heap implementation.

The heap size is configured at link time:

```bash
make -C sdk/faeulibc HEAP_SIZE=1024
```

The default heap size is `1024` bytes (1 KiB).

Heap support can also be disabled explicitly:

```bash
make -C sdk/faeulibc NO_HEAP=true
```

Setting `HEAP_SIZE=0` implicitly enables `NO_HEAP=true`.

## Integration Notes

- `stdout` and `stderr` are split in the glue layer, but the reference
  semihosting backend currently routes them to the same channel.
- `stdin` uses a simple character-by-character `read_hs()`.
- `_exit()` is minimal and does not yet try to expose a stable FAE-side
  termination ABI.
- `fopen`, `fdopen`, `freopen`, `open`, `close`, `read`, `write`, and `lseek`
  are intentionally not provided in this starter.
