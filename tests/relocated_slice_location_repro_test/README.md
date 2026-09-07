# Relocated Slice Location Repro

This crate reproduces a Rust relocation pattern emitted by ordinary slice
bounds checks. The expression below generates read-only panic location metadata
(`.rodata..Lanon...`) that contains absolute pointers to other ROM data:

```rust
out[..payload.len()].copy_from_slice(payload);
```

`build_fae_rust --elf` must move that metadata to the writable runtime block
because the FAE runtime only exports and applies `R_ARM_ABS32` relocations from
`.rom.ram`. Leaving such metadata in `.rom` produces an apparently valid FAE
with unpatched absolute pointers.
