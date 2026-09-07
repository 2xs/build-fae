# Rust CRT0

`rustrt0-arm` is a Rust implementation of the XiPFS-compatible CRT0 startup logic for ARM-state targets.

It is responsible for:

- validating the FAE footer magic/version
- locating the metadata appended after the CRT0 binary
- relocating `.got`, `.rom.ram`, and `.ram`
- patching runtime pointers from relocation entries
- calling the relocated entrypoint itself
- setting the relocated static base / GOT base in `r9` before that call

## Build

From the workspace root:

```bash
cargo rustrt0-arm
```

Or explicitly:

```bash
cargo run -p xtask -- rustrt0-arm
```

The generated binary is:

- `build/rustrt0-arm.bin`

The corresponding ELF with symbols is kept under:

- `target/armv7a-none-eabi/release/rustrt0-arm`

## Implementation Notes

- Entry point: [`src/rustrt0_arm.rs`](src/rustrt0_arm.rs)
- Linker script: [`link.ld`](link.ld)
- The linker script exports `__metadataOff`, which marks the offset where FAE metadata starts immediately after the CRT0 binary image.

## Requirements

The build currently expects:

- `cargo`
- `arm-none-eabi-ld`
- `arm-none-eabi-objcopy`

## License

GPL-3.0-only. See [../LICENSE](../LICENSE).
