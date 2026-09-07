# FAE Rust Runtime Starter

Minimal Rust starter for building an FAE payload with:

- a local `no_std` runtime library
- a runtime entry point `start() -> i32`
- a tiny Rust-side bridge from `start()` to a user-facing `fae_main()`
- semihosting-backed `print!` / `println!` / `eprint!` / `eprintln!`
- minimal console input with `read_char()` and `read_buffer()`
- a tiny first-fit global allocator for `alloc` with a 1024-byte default heap
- a dedicated FAE linker script exporting the section size symbols expected by `build_fae`
- a Cargo-first workflow driven by the workspace `xtask`

## Layout

```text
sdk/fae-rustrt/
  .cargo/config.toml    Local target and build-std configuration
  src/lib.rs            Minimal Rust runtime
  examples/helloworld/  Example user crate linked with the runtime
  link.ld               FAE section layout for the payload ELF
  Cargo.toml            Local starter crate
```

## Current Scope

This starter is intentionally small.

It provides:

- `start()` as the FAE runtime entry point
- `entry!(fae_main)` to bind a user Rust function to the runtime
- `print!` / `println!` / `eprint!` / `eprintln!` through semihosting `SYS_WRITEC`
- `read_char()` / `read_buffer()` through semihosting `SYS_READC`
- a fixed-size first-fit allocator exposed as `#[global_allocator]`
- a panic handler
- an example `helloworld/` crate using `Box`

It does not yet provide:

- argument decoding from the current host context
- filesystem bindings
- a stable termination ABI
- a host-backed allocator via the future FAE `core` syscall family

## Build

From the repository root:

```bash
cargo run -p xtask -- fae-rustrt
```

This produces:

- `sdk/fae-rustrt/build/helloworld.elf`

## Package As FAE

From the repository root:

```bash
cargo run -p xtask -- fae-rustrt fae
```

This additionally produces:

- `sdk/fae-rustrt/build/helloworld.fae`
- `sdk/fae-rustrt/build/helloworld.gdbinit`

## Check Rust Codegen

To verify that Rust is emitting the expected ARM ABI model before the final
link step:

```bash
cargo run -p xtask -- fae-rustrt check
```

This compiles `fae_rustrt` as an object file only, then checks for:

- `Tag_ABI_PCS_R9_use: SB`
- `Tag_ABI_PCS_RW_data: SB-relative`
- `Tag_ABI_PCS_RO_data: PC-relative`
- at least one `R_ARM_SBREL32` relocation

That check is meant to validate Rust code generation, independently from the
current FAE linker/runtime limitations.

## Hello World Example

The starter ships a user payload example crate in:

- `sdk/fae-rustrt/examples/helloworld/`

The application crate depends on `fae_rustrt`, imports only `entry`, and uses
the exported `println!` macro directly:

```rust
use fae_rustrt::entry;

entry!(app_main);

fn app_main() -> i32 {
    println!("Hello World!");
    let ch = fae_rustrt::read_char();
    println!("Read: {}", ch as char);
    0
}
```

On this `no_std` target, that `println!` is provided by `fae_rustrt`, not by
`std`.

## Runtime Shape

The runtime library exports `start()`.

The user application provides a normal Rust function and binds it with:

```rust
use fae_rustrt::entry;

fn app_main() -> i32 {
    println!("hello");
    0
}
```

So the execution model becomes:

1. bundled FAE `rt0`
2. `fae_rustrt::start()`
3. user `main()`

## Customization Path

The intended first customizations are:

1. replace `examples/helloworld/` with a real payload crate
2. switch console output from direct semihosting to the future FAE `core` write path
3. replace the local heap with `fae_core_ram_alloc` / `fae_core_ram_free`

For now, the starter stays intentionally small and self-contained so the
runtime surface remains easy to evolve.
