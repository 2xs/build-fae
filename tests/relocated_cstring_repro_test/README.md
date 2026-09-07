Flow of events:

```shell
cargo clean
cargo build
cargo run --bin build_fae_rust -- \
      --manifest-path tests/relocated_cstring_repro_test/Cargo.toml \
      --elf --profile dev
cargo run --bin build_fae -- --verbose \
      tests/relocated_cstring_repro_test/build/relocated_cstring_repro_test.elf
```

This case is now expected to work in both `--profile dev` and the default
release profile.

It used to expose three distinct issues:

- the Thumb SB-relative rewrite path did not handle high registers like `r12`/`ip`
- `build_fae` rejected executable `R_ARM_THM_MOVW_BREL_NC` / `R_ARM_THM_MOVT_BREL`
  relocations even though they are already SB-relative
- optimized Thumb code may interleave several `MOVW`/`MOVT` address
  materializations before their matching `ADD rN, pc` instructions

## Historical bug inspection

```shell
# Find the corrupted ADD where MOVW says ip but ADD says r4:
arm-none-eabi-objdump -d tests/relocated_cstring_repro_test/build/relocated_cstring_repro_test.elf | grep 'r4, r9' -B2
```

Before the fix, the build had wrong locations such as `0x2bd2` and `0x52b2`:

```
2bca:   f240 2cf0   movw ip, #752    
2bce:   f2c0 0c00   movt ip, #0
2bd2:   444c        add  r4, r9      ← should be 44cc (add ip, r9)

52aa:   f240 6cb0   movw ip, #1712   ← same pattern
52ae:   f2c0 0c00   movt ip, #0
52b2:   444c        add  r4, r9      ← same
```

## Fix summary

The high-register encoding bug was fixed in `crates/fae-elf/src/elf.rs`, and the
`BREL` executable relocations are now accepted by the validator because they are
already SB-relative.

The release-profile interleaving bug is also fixed in `crates/fae-elf/src/elf.rs`:
the SB-relative rewrite path now searches a short bounded window for the
matching `ADD rN, pc` instead of assuming that it immediately follows the
relocated `MOVT`.

## Historical release-profile error

```shell
cargo clean
cargo build
cargo run --bin build_fae_rust -- \
      --manifest-path tests/relocated_cstring_repro_test/Cargo.toml \
      --elf

Error: unsupported Thumb address materialization for writable runtime symbol .Lanon.1da254df4580e80a62fc836494f2f59d.4; expected `add r2, pc` but found 0x4478
```
