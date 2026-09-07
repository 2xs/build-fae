# FAE Rust Runtime Notes

## Operational Summary

For the current repository state, the shortest correct summary is:

- the GNU ARM linker path fails on `R_ARM_SBREL32`
- the `rust-lld` path works for the same Rust `ROPI-RWPI` payloads
- the recommended flow today is:
  - [`build_fae_rust`](build_fae_rust.md)
  - or its wrapper `cargo run -p xtask -- fae-rustrt`
- the repository repros for relocated descriptors, trait objects, and cstring
  formatting now build successfully through that flow

So, when experimenting with the Rust runtime starter:

- do not treat `arm-none-eabi-gcc` / GNU `ld` as the reference final linker
- do treat `rust-lld` as the known-good linker path
- do treat `xtask fae-rustrt` as a convenience wrapper around that `rust-lld`-based flow
- do pass an explicit packaging mode to `build_fae_rust`:
  - `--elf`
  - `--fae`
  - or `bootable <board>`

Current alignment-related packaging options are:

- `--align_payload <bytes>` to align the payload partition offset
- `--align_size <bytes>` to align the final `.fae` file size

For example:

```bash
cargo run --bin build_fae_rust -- \
  --manifest-path sdk/fae-rustrt/examples/helloworld/Cargo.toml \
  --align_payload 32 \
  --align_size 32 \
  --fae
```

The rest of this note explains how that conclusion was reached.

## Goal

This note tracks the compilation and linking experiments made for the Rust
runtime starter, with a focus on:

1. a Rust/LLVM-only toolchain path
2. the most promising compilation scheme observed so far
3. what currently violates the intended `ROPI` / `RWPI` model for FAE payloads

## Short Summary

- The most promising Rust-only scheme is:
  - `cargo +nightly -Z build-std=core,alloc,compiler_builtins`
  - `rust-lld`
  - `-C relocation-model=ropi-rwpi`
- In that scheme, LLVM clearly uses `r9` as the static base register for `RWPI`.
- The final linked ELF can be produced cleanly with `rust-lld`.
- `rust-lld --emit-relocs` can preserve relocation records in the final ELF,
  including the exact `.rel.rom` entries corresponding to the problematic
  absolute Thumb function pointers.
- The runtime crash seen after packaging is not best explained by `r9`
  clobbering.
- The strongest observed cause is different: the final ELF still contains
  absolute Thumb function pointers in `.rom`, and in the default final link
  those relocation entries are not kept for a second relocation step after the
  bootable startup is prepended.
- With the current GNU ARM linker path, `ROPI` links, but `RWPI` and
  `ROPI-RWPI` fail on `R_ARM_SBREL32`.

## Rust/LLVM-Only Research

The relevant upstream sources found during the investigation were:

- Cargo `build-std`:
  - <https://doc.rust-lang.org/cargo/reference/unstable.html#build-std>
- Rust codegen options:
  - <https://doc.rust-lang.org/rustc/codegen-options/index.html>
- GCC ARM PIC options, useful as a reference for the lower-level concepts:
  - <https://gcc.gnu.org/onlinedocs/gcc-3.4.5/gcc/ARM-Options.html>
- ARM ABI reference repository:
  - <https://github.com/ARM-software/abi-aa>

Useful observations from the Rust side:

- `rustc` exposes:
  - `-C relocation-model`
  - `-C code-model`
  - `-C jump-tables`
  - `-C llvm-args`
- `rustc --print relocation-models` reports:
  - `static`
  - `pic`
  - `pie`
  - `dynamic-no-pic`
  - `ropi`
  - `rwpi`
  - `ropi-rwpi`
- `rustc --print code-models` reports:
  - `tiny`
  - `small`
  - `kernel`
  - `medium`
  - `large`

Useful observations from the LLVM side:

- `rustc -C llvm-args=--help-hidden --target thumbv7em-none-eabi` shows the
  relocation models `ropi`, `rwpi`, and `ropi-rwpi`.
- No clearly exposed LLVM flag was found through `rustc` help output to switch
  the static base register from `r9` to `r10`.
- GCC documents `-msingle-pic-base` and `-mpic-register`, but no equivalent
  stable `rustc` flag was found in the Rust-only toolchain path during this
  investigation.

## Promising Scheme: Rust-Only, `rust-lld`, `ropi-rwpi`

The cleanest end-to-end Rust/LLVM build found so far was:

```bash
cargo +nightly-aarch64-apple-darwin build \
  -Z build-std=core,alloc,compiler_builtins \
  -Z build-std-features=compiler-builtins-mem \
  --manifest-path sdk/fae-rustrt/examples/helloworld/Cargo.toml \
  --release \
  --target thumbv7em-none-eabi \
  --target-dir /tmp/fae-rust-lld-clean \
  --config 'target.thumbv7em-none-eabi.linker="<path-to-rust-lld>"' \
  --config 'target.thumbv7em-none-eabi.rustflags=["-C","relocation-model=ropi-rwpi","-C","panic=abort","-C","link-arg=-Tsdk/fae-rustrt/link.ld","-C","link-arg=--gc-sections","-C","link-arg=--no-undefined"]'
```

This build:

- compiled `core`, `alloc`, and `compiler_builtins` from source
- linked the final ELF without tripping:
  - the linker
  - `build_fae`
  - `read_fae`

At this point, this was the most promising configuration.

## What This Scheme Proves

### 1. LLVM really uses `r9` as the `RWPI` static base

The final ELF built with `rust-lld` reports:

- `Tag_ABI_PCS_R9_use: SB`
- `Tag_ABI_PCS_RW_data: SB-relative`
- `Tag_ABI_PCS_RO_data: PC-relative`

This was observed on:

- `/tmp/fae-rust-lld-clean/thumbv7em-none-eabi/release/fae_rustrt_helloworld`
- `/tmp/fae-rust-lld-clean/thumbv7em-none-eabi/release/deps/fae_rustrt_helloworld-85fe5561235fa2ef.fae_rustrt_helloworld.922cf19ac73839ba-cgu.0.rcgu.o`

The final linked code also contains many explicit `r9`-relative accesses, for
example:

- `add.w r0, r9, r8`
- `add.w r1, r9, r8`
- `str.w r1, [r9, r0]`

So in the promising Rust-only scheme, the evidence points strongly to:

- `r9` is the chosen static base register
- LLVM is not treating `r9` as a freely allocatable general register there

### 2. The runtime crash in that scheme is not best explained by `r9` clobbering

When the `rust-lld` ELF was packaged into a bootable FAE and run under QEMU,
the trace at the point of failure still showed a coherent `r9` value:

- `r9 = 0x20000000`

The failure happened deeper in `core::fmt`, and the strongest evidence did not
point to a lost static base register. In other words:

- the crash is real
- but the observed crash in this scheme is not best explained by register
  allocation accidentally reusing `r9`

That does not prove that `r10` is impossible or unnecessary, but it does mean
the observed failure should not currently be attributed first to `r9`
allocation.

## What Violates `ROPI` / `RWPI` for FAE

The most important default-link result of the investigation is this:

- the final `rust-lld` ELF has no relocation sections unless relocations are
  explicitly preserved
- yet `.rom` still contains absolute Thumb function pointers
- those pointers are valid only before the bootable startup shifts the payload

This is the clearest currently observed violation of the FAE model.

### Evidence

The final `rust-lld` ELF contained no relocations:

```text
There are no relocations in this file.
```

Yet scanning `.rom` found fixed function pointers such as:

- `.rom + 0xba8` = `0x0b15` = `start + 1`
- `.rom + 0xd9c` = `0x0b05` = `Console::write_str + 1`
- `.rom + 0xda0` = `0x0a35` = `ErrorConsole::write_char + 1`
- `.rom + 0xda4` = `0x0a1d` = `ErrorConsole::write_fmt + 1`
- `.rom + 0xdbc` = `0x0aed` = `Console::write_fmt + 1`

For the `mps2-an385` bootable image, the prepended startup size was:

- `0x298` bytes

So those values should have become:

- `start + 1`:
  - stored: `0x0b15`
  - expected after boot offset: `0x0dad`
- `Console::write_str + 1`:
  - stored: `0x0b05`
  - expected after boot offset: `0x0d9d`
- `ErrorConsole::write_char + 1`:
  - stored: `0x0a35`
  - expected after boot offset: `0x0ccd`

This means:

- the final linked image is not double-relocatable in the way FAE currently
  needs
- `ROPI` / `RWPI` is being advertised in ABI attributes
- but the final linked ELF still freezes absolute function pointers in `.rom`
  without keeping relocations for a second relocation step

## `rust-lld --emit-relocs`

The most important follow-up experiment was to rebuild the same Rust-only
scheme with:

- `rust-lld`
- `-C relocation-model=ropi-rwpi`
- `-C link-arg=--emit-relocs`

Local `rust-lld` help confirms:

```text
--emit-relocs           Generate relocations in output
-q                      Alias for --emit-relocs
```

### Result

This final ELF now contains:

- `.rel.rom`

and the relocation entries include the exact problematic offsets previously
observed in `.rom`.

Examples from the final linked ELF:

- `.rel.rom + 0x0ba8`:
  - `R_ARM_ABS32` -> `start`
- `.rel.rom + 0x0d9c`:
  - `R_ARM_ABS32` -> `Console::write_str`
- `.rel.rom + 0x0da0`:
  - `R_ARM_ABS32` -> `ErrorConsole::write_char`
- `.rel.rom + 0x0da4`:
  - `R_ARM_ABS32` -> `ErrorConsole::write_fmt`
- `.rel.rom + 0x0dbc`:
  - `R_ARM_ABS32` -> `Console::write_fmt`

The same final ELF also still contains:

- `R_ARM_SBREL32`
- `R_ARM_REL32`

So in this Rust-only configuration, the toolchain is not “forgetting” all
relocation intent. It can preserve it in the final ELF when asked to do so.

### What This Changes In The Diagnosis

This is a key distinction:

- without `--emit-relocs`, the final ELF loses the relocation metadata FAE
  would need for a second relocation pass
- with `--emit-relocs`, the final ELF keeps that metadata, including the
  absolute pointers in `.rom`

That means the current evidence points less to “Rust/LLVM violates ROPI/RWPI at
object generation time” and more to:

- the final executable link normally resolving and discarding relocation
  information
- while FAE needs that information to survive up to packaging time

In other words, the current Rust/LLVM chain can already describe the
problematic `.rom` pointers precisely, but the FAE pipeline does not yet use
that information.

## Current FAE Limitation Exposed By `--emit-relocs`

Packaging the `rust-lld --emit-relocs` ELF with the current `build_fae` still
produces:

- a `.fae` with no relocation entries
- the same boot-time hard fault on QEMU

The reason is straightforward in the current implementation:

- `build_fae` only exports `.rel.rom.ram`
- it does not currently export `.rel.rom`

So even when the Rust/LLVM-linked ELF retains the needed `.rom` relocations,
the current FAE packaging step drops them.

An important nuance is that the problematic `.rel.rom` entries observed in the
Rust-linked ELF are already of type:

- `R_ARM_ABS32`

That is the relocation type already supported by `build_fae` for
`.rel.rom.ram`.

So the current gap is not primarily “Rust emits an unknown relocation kind”.
The more immediate gap is:

- `build_fae` only looks at `.rel.rom.ram`
- `build_fae` currently rejects relocation targets outside the writable area

For the concrete problematic function-pointer slots in `.rom`, the relocation
kind itself is already familiar.

This was confirmed by:

- the final ELF containing `.rel.rom`
- the generated FAE still showing `Relocation entries : none`
- QEMU still hard-faulting with the bootable image

## Experiments With the Current GNU ARM Linker

The following experiments were run while keeping the current GNU ARM linker
path.

### Base case: `ropi-rwpi`

Command shape:

- `cargo +nightly ...`
- `-Z build-std=core,alloc,compiler_builtins`
- `-C relocation-model=ropi-rwpi`
- GNU ARM linker

Observed result:

- link failure
- `dangerous relocation: unsupported relocation`

The failures were consistently attached to `RWPI` accesses, for example:

- `__rust_alloc`
- `__rust_dealloc`
- `runtime_init`

### `jump-tables=no`

Observed result:

- no meaningful change
- same linker failure pattern

### `lto=false`

Observed result:

- no fix
- the exact symbol names changed
- the same kind of `dangerous relocation` failures remained

### `code-model=tiny`

Observed result:

- rejected on this target in this setup

### Relocation model matrix

Observed result:

- `ropi`:
  - links successfully with the GNU linker
- `rwpi`:
  - link failure
- `ropi-rwpi`:
  - link failure

This is an important result:

- the current GNU linker path accepts the `ROPI` side
- it rejects the `RWPI` side

## Rust-Only Matrix Beyond `ROPI-RWPI`

To check whether the absolute pointers in `.rom` were specific to
`ropi-rwpi`, a small Rust-only matrix was also tested with:

- `rust-lld`
- `--emit-relocs`

Observed result:

- `pic`:
  - links successfully
  - still produces `R_ARM_ABS32` entries in the final ELF
  - no `R_ARM_SBREL32`
- `pie`:
  - links successfully
  - still produces `R_ARM_ABS32` entries in the final ELF
  - no `R_ARM_SBREL32`

This is another useful narrowing result:

- the presence of absolute `.rom` function pointers is not unique to
  `ropi-rwpi`
- it is a more general property of how the final linked executable represents
  certain function-pointer-bearing read-only data

## Feasibility Of Preserving `.rel.rom`

The next question was whether the FAE pipeline could preserve `.rel.rom`
through the final Rust/LLVM link, then consume those entries during packaging.

### Result

This is technically feasible at the ELF level today.

With:

- `rust-lld`
- `-C relocation-model=ropi-rwpi`
- `-C link-arg=--emit-relocs`

the final linked ELF retains:

- `.rel.rom`

and the entries include both code-side relocations and read-only data
relocations.

Important examples from the final linked ELF:

- `.rel.rom + 0x0ba8`:
  - `R_ARM_ABS32` -> `start`
- `.rel.rom + 0x0d9c`:
  - `R_ARM_ABS32` -> `Console::write_str`
- `.rel.rom + 0x0da0`:
  - `R_ARM_ABS32` -> `ErrorConsole::write_char`
- `.rel.rom + 0x0da4`:
  - `R_ARM_ABS32` -> `ErrorConsole::write_fmt`
- `.rel.rom + 0x0dbc`:
  - `R_ARM_ABS32` -> `Console::write_fmt`

This means the information FAE would need is not inherently lost by Rust/LLVM.
It is only discarded in the default final-link configuration.

An important nuance is that `.rel.rom` is not directly a ready-made FAE
relocation table. It contains a mix of:

- `R_ARM_THM_CALL`
- `R_ARM_THM_JUMP24`
- `R_ARM_REL32`
- `R_ARM_SBREL32`
- `R_ARM_ABS32`

For a second relocation pass after FAE packaging, the clearly interesting
subset is not the whole raw `.rel.rom`, but primarily the `R_ARM_ABS32` slots
that freeze absolute addresses inside read-only data.

### Where The Current FAE Packaging Stops

The current `build_fae` implementation only exports:

- `.rel.rom.ram`

and it only accepts relocation offsets in the writable area. For that reason,
the Rust/LLVM final ELF can already carry the needed `.rel.rom` data, while the
current FAE packaging still drops it.

This is an encouraging result, because the problematic `.rel.rom` entries
observed so far are already of type:

- `R_ARM_ABS32`

That is the same relocation kind already supported for `.rel.rom.ram`.

So preserving `.rel.rom` up to packaging time is feasible, and extending
`build_fae` to export it looks substantially simpler than inventing a new Rust
code-generation mode.

The likely right shape is therefore:

- keep `.rel.rom` in the final ELF
- filter it during packaging
- export only the subset that still matters after whole-section rebasing

## Feasibility Of Rewriting ROM Accesses To `r9` / `r10`

The more speculative idea was:

1. detect code that accesses problematic read-only slots
2. rewrite those accesses so they go through `r9` / `r10`
3. merge the corresponding slots with the writable relocation machinery

### What The Rust Binary Actually Contains

The strongest currently observed `R_ARM_ABS32` offenders are not instruction
operands inside `.text`. They are read-only data words in `.rom`, for example:

- a 4-byte object at `0x0ba8` holding `start + 1`
- function-pointer-bearing read-only objects around `0x0d9c..0x0dbc`

Dumping `.rom` around those offsets shows plain data words, not instructions:

```text
0x0d90: 00000000 00000000 01000000 050b0000
0x0da0: 350a0000 1d0a0000 00000000 00000000
0x0db0: 01000000 050b0000 350a0000 ed0a0000
```

Those values correspond to the exact `R_ARM_ABS32` function-pointer targets
found in `.rel.rom`.

So the first important limitation is:

- the main problem is not only "instructions with the wrong base register"
- it is also read-only tables whose contents themselves require relocation

### Thumb Encoding Constraint

A second experiment compared actual Thumb encodings:

- `ldr r0, [pc, #imm]`:
  - 16-bit
- `ldr r0, [r7, #imm]`:
  - 16-bit
- `ldr r0, [r9, #imm]`:
  - 32-bit
- `ldr r0, [r10, #imm]`:
  - 32-bit
- `str r0, [r7, #imm]`:
  - 16-bit
- `str r0, [r9, #imm]`:
  - 32-bit
- `str r0, [r10, #imm]`:
  - 32-bit

Likewise for register-offset forms:

- low-register base forms can be 16-bit
- `r9` / `r10` base forms become 32-bit in the tested cases

This makes an in-place generic binary rewrite much less attractive:

- a 16-bit access cannot in general be replaced by an `r9` / `r10` access of
  the same size
- any transformation would need instruction-size awareness, code-motion
  tolerance, and probably local padding or thunking

### Practical Consequence

This does not make binary rewriting impossible in principle, but it strongly
suggests it is not the best first move.

The currently more feasible path is:

1. keep `.rel.rom` in the final Rust/LLVM ELF
2. extend `build_fae` to export `.rel.rom` in addition to `.rel.rom.ram`
3. decide at the FAE/runtime level how ROM-targeted relocations are applied

By contrast, a build-time pass that rewrites `pc`-relative accesses into
`r9` / `r10`-relative ones would need:

- a Thumb instruction decoder/rewriter
- relocation-to-instruction provenance tracking
- special handling for size expansion from 16-bit to 32-bit encodings
- separate handling for read-only data tables, which are not instruction
  accesses at all

So this path looks technically possible only as a much larger binary-rewriter
project, not as a small incremental extension to `build_fae`.

## Experimental FAE Export With A Second ROM Relocation Table

To validate the packaging side without solving runtime relocation yet, an
experimental FAE layout was tried with:

- the existing writable relocation table first
- then a second ROM-address relocation table

The second table uses the shape:

- count word
- then `count` pairs of:
  - write offset
  - target address value

This was prototyped with:

- `build_fae --experimental-rom-relocations`
- `read_fae`

### Result On The Rust `rust-lld --emit-relocs` ELF

Using the previously generated Rust ELF with preserved `.rel.rom`, the
experimental packaging step reported:

- no `.rel.rom.ram`
- 8 exported ROM-address relocation pairs

and `read_fae` displayed:

```text
- Writable relocation entries : none.
- ROM relocation address entries : [start : @668 bytes , end : @731 bytes] (size : 64 bytes)
    - Count : 8
    - [0] write @2984 bytes <- address 2837 bytes
    - [1] write @3240 bytes <- address 0 bytes
    - [2] write @3484 bytes <- address 2821 bytes
    - [3] write @3488 bytes <- address 2613 bytes
    - [4] write @3492 bytes <- address 2589 bytes
    - [5] write @3508 bytes <- address 2821 bytes
    - [6] write @3512 bytes <- address 2613 bytes
    - [7] write @3516 bytes <- address 2797 bytes
```

This does not yet make the resulting firmware runnable, because the startup
code still does not consume that second table. But it proves that:

- the final Rust ELF can carry the relevant `.rel.rom` information
- `build_fae` can extract the representable `R_ARM_ABS32` subset
- `read_fae` can display it in a way that is directly inspectable

## What `ROPI`-Only Means

`ROPI`-only was tested as a diagnostic.

The GNU-linked `ROPI` ELF:

- linked successfully
- packaged successfully as bootable FAE
- did not hard-fault immediately under QEMU

This does not solve the actual runtime goal, because `fae-rustrt` needs mutable
runtime state and allocator support. But it helps isolate the issue:

- the hard part for the current GNU linker is the `RWPI` side
- not the `ROPI` side

## Current Conclusions

1. The most promising Rust-only compilation scheme remains:
   - `build-std`
   - `rust-lld`
   - `ropi-rwpi`
2. In that scheme, LLVM clearly uses `r9` as the static base register.
3. The observed runtime crash in that scheme is not best explained by `r9`
   being treated as a normal general-purpose register.
4. The clearest currently observed violation of the FAE relocation model is:
   - absolute Thumb function pointers frozen in `.rom`
   - no relocation entries preserved in the final ELF
   - bootable offset then makes those pointers wrong
5. With the current GNU ARM linker, the specific blocking point is the `RWPI`
   half:
   - `ROPI` links
   - `RWPI` and `ROPI-RWPI` do not

## Open Questions

- Whether a custom Rust target can steer LLVM toward a usable static-base model
  that fits FAE better without patching LLVM.
- Whether any Rust/LLVM-only flag path exists to move the static base register
  away from `r9` and onto `r10`.
- Whether the final absolute `.rom` pointers can be eliminated by compilation
  choices alone, or whether this would require:
  - linker changes
  - preserved relocations
  - or toolchain/backend changes

## Experiment: Move Relocatable RO Tables Into `.rom.ram`

The next experiment was to stop treating all Rust-generated read-only tables as
true ROM data and instead move the suspicious ones into the ROM-backed writable
payload block.

The idea was:

- keep obvious immutable code and strings in `.rom`
- move likely relocatable RO tables into the beginning of `.rom.ram`
- let the final ELF produce `.rel.rom.ram` so the existing writable relocation
  machinery can handle them

### Object File Panel

Host-side intermediates for `build_fae` and `read_fae` are not useful for this
question on macOS because they are Mach-O host objects, not ARM ELF objects.
For a representative ARM/ELF panel, the following objects were inspected
instead:

- `sdk/fae-rustrt/examples/helloworld/target/thumbv7em-none-eabi/release/deps/fae_rustrt_helloworld-0ce08586262fd0b5.fae_rustrt_helloworld.922cf19ac73839ba-cgu.0.rcgu.o`
- `sdk/fae-rustrt/target/thumbv7em-none-eabi/debug/incremental/fae_rustrt-3tiz4mgb8rtzz/s-hgpst6rena-0v0fdn2-404ne5zmim7zyq99q29kuyjh4/e3u4jqkewjk9dkucmg39xzozb.o`

The first object shows the pattern in a compact form:

- `.rodata._RNv...___FAE_KEEP_START`
- `.rel.rodata._RNv...___FAE_KEEP_START`
- `.rodata..Lanon...`
- `.rel.rodata..Lanon...`

The second object shows the same pattern at scale:

- dozens of `.rodata..Lanon.*`
- each paired with `.rel.rodata..Lanon.*`
- in this sample, the `.rel.rodata.*` relocations were all `R_ARM_ABS32`

Typical examples observed:

- `.rel.rodata._RNv...___FAE_KEEP_START`
  - `R_ARM_ABS32 -> start`
- `.rel.rodata..Lanon....1`
  - `R_ARM_ABS32 -> Console::write_str`
  - `R_ARM_ABS32 -> ErrorConsole::write_char`
  - `R_ARM_ABS32 -> ErrorConsole::write_fmt`
- many `.rel.rodata..Lanon.*`
  - `R_ARM_ABS32 -> .rodata.str1.1`

So the useful conclusion from the object-level scan is:

- on Rust ARM objects, the problematic read-mostly tables are not emitted as
  `.data.rel.ro*`
- they mostly appear as `.rodata.*` input sections with matching
  `.rel.rodata.*`

### `ariel-os` ABS32-Only Collection

To avoid overfitting the heuristic to the local starter alone, a second pass was
run on the already-built ARM/Thumb objects found under `../ariel-os/target/`,
this time filtered to keep only target sections that receive at least one
`R_ARM_ABS32`.

The collection used `check_elf_rel --abs32-only` and the raw / unique outputs
were written to:

- `/tmp/o.txt`
- `/tmp/o.unique.txt`

The compiled ARM/Thumb corpus available at that moment was small, but the
deduplicated result was still useful:

```text
.debug_aranges
.debug_frame
.debug_info
.debug_line
.debug_loc
.debug_ranges
.rodata..Lanon.fcaa0e5fb817867662f8e7d762dd6b4f.2
.rodata..Lanon.fcaa0e5fb817867662f8e7d762dd6b4f.3
.rodata..Lanon.fcaa0e5fb817867662f8e7d762dd6b4f.5
.rodata..Lanon.fcaa0e5fb817867662f8e7d762dd6b4f.6
.rodata..Lanon.fcaa0e5fb817867662f8e7d762dd6b4f.8
```

So, with the current discovery pass restricted to `R_ARM_ABS32`, the practical
rule becomes:

- keep `debug_*`
- keep `.rodata..Lanon.*`

This is not yet a proof that those are the only families that will ever appear
in larger Rust builds, but it is the current observed set worth preserving in
the automatically generated linker script.

### Linker Script Heuristic

Because the linker script cannot directly ask "does this input section have a
relocation section?", a heuristic was used in
`sdk/fae-rustrt/link.ld`:

- keep in `.rom`:
  - `.text*`
  - `.rodata`
  - `.rodata.str*`
  - `.rodata.cst*`
- move to the beginning of `.rom.ram`:
  - `.data.rel.ro`
  - `.data.rel.ro.*`
  - `.rodata.*`
- then place normal `.data*` after that moved RO subset

The linker script also exposes a logical split inside `.rom.ram`:

- `__rom_ram_ro_start`
- `__rom_ram_ro_end`
- `__rom_ram_ro_size`
- `__rom_ram_rw_start`
- `__rom_ram_rw_end`
- `__rom_ram_rw_size`

This is only a heuristic:

- it definitely catches the small Rust tables that previously lived in `.rom`
- it also over-approximates by moving some other `.rodata.*` payloads into the
  ROM-backed writable block

For the current experiment, that over-approximation is acceptable.

### Result On The Final ELF

With:

- `cargo +nightly`
- `-Z build-std=core,alloc,compiler_builtins`
- `rust-lld`
- `-C relocation-model=ropi-rwpi`
- `-C link-arg=--emit-relocs`

the final linked ELF now contains:

- `.rom`
- `.got`
- `.ARM.exidx`
- `.rom.ram`
- `.rel.rom`
- `.rel.rom.ram`

Most importantly, the ABS32 relocations that had previously remained in
`.rel.rom` for the `println!` machinery moved into `.rel.rom.ram`.

Observed `.rel.rom.ram` entries included:

- `0x0ca0 -> start`
- `0x0cb8 -> .rom`
- `0x0dac -> Console::write_str`
- `0x0db0 -> ErrorConsole::write_char`
- `0x0db4 -> ErrorConsole::write_fmt`
- `0x0dc4 -> Console::write_str`
- `0x0dc8 -> ErrorConsole::write_char`
- `0x0dcc -> Console::write_fmt`

This is a strong signal that the basic idea is technically sound:

- by changing section placement, the problematic read-mostly Rust tables can be
  turned into ordinary writable-relocation inputs

### Limitation Revealed By Packaging

Packaging with the current `build_fae` still fails afterward.

The immediate failure is:

```text
cannot remap .rom.ram offset 1952670066
```

This does not invalidate the linker-script experiment. It reveals a separate
assumption in the current packaging path:

- `build_fae` reconstructs the raw partition layout from the logical trio
  `.rom`, `.got`, `.rom.ram`
- but the ELF still has a distinct `.ARM.exidx` output section between `.got`
  and `.rom.ram`
- so relocation offsets coming from the ELF use addresses that include that
  extra span
- while `build_fae` remaps them as if `.rom`, `.got`, and `.rom.ram` were
  already tightly packed

In other words:

- the linker-script heuristic succeeds at pushing the suspicious tables into
  `.rom.ram`
- but the current `build_fae` offset-remapping logic is still too strict about
  the ELF section topology

So the next technical step, if this direction is kept, is not another Rust
codegen change. It is teaching `build_fae` to cope with the actual exported ELF
layout, notably the separate `.ARM.exidx` span.

## Dedicated Rust App Flow

To keep the runtime starter small, the orchestration was split into three
separate areas:

- runtime only:
  - `sdk/fae-rustrt`
- reusable app build tool:
  - `crates/build-fae-rust`
- project skeleton and derived example:
  - `sdk/templates/rust-fae-app`
  - `examples/rust-fae-helloworld`

The current flow is:

1. build a first Rust ELF with:
   - `cargo +nightly`
   - `-Z build-std=core,alloc,compiler_builtins`
   - `rust-lld`
   - `-C relocation-model=ropi-rwpi`
   - `-C link-arg=--emit-relocs`
2. inspect that discovery ELF
3. generate a throwaway linker script
4. relink with that script passed as a pre-link argument
5. hand the final ELF to `build_fae`

The corresponding linker script generator is now:

- `generate_fae_rust_ld`

and the app wrapper tool is:

- `build_fae_rust`

### Current Placement Heuristic

The current generator is still intentionally conservative.

Observed from the discovery ELF:

- `.rodata` receives `R_ARM_ABS32`
- `.text` does not need to move
- `.bss` remains ordinary RAM

So the generated script currently:

- keeps `.text*` in `.rom`
- moves `.rodata*` into `.rom.ram`
- keeps `.bss*` in `.ram`
- discards `.ARM.exidx*` and `.ARM.extab*`

That last point is deliberate for the current minimal runtime profile:

- `panic=abort`
- no unwinding support expected at runtime

It avoids the section type mismatch seen with `rust-lld` when trying to merge
`SHT_ARM_EXIDX` into a normal `PROGBITS` FAE section.

### Observed Result On `rust-fae-helloworld`

Validated with:

```bash
cargo run --manifest-path examples/rust-fae-helloworld/xtask/Cargo.toml -- --fae
cargo run --manifest-path examples/rust-fae-helloworld/xtask/Cargo.toml -- bootable mps2-an385
```

The final ELF now exports:

- `__rom_size = 2984`
- `__rom_ram_size = 536`
- `__got_size = 0`
- `__ram_size = 1040`

and `build_fae` exports:

- `.rom` successfully
- `.rom.ram` successfully
- `.rel.rom.ram` with 8 `R_ARM_ABS32` entries

`check_elf_rel` on the final ELF reports:

- `.rel.rom` still present with 77 entries
- `.rel.rom.ram` present with 8 entries

That means the dedicated Rust app flow is now good enough to:

- build a normal Rust `no_std` app against `fae-rustrt`
- generate a FAE-compatible final ELF
- package both hosted and bootable `.fae` images

### Remaining Limitation

The discovery pass still works at the granularity of the first linked ELF, not
yet at the granularity of every original `.o`.

So the current generator over-approximates:

- it moves the whole `.rodata*` family into `.rom.ram`

This is acceptable for the current prototype because it produces a correct FAE
layout and relocation table, but it is not yet the fine-grained scheme
originally envisioned.

## Object-Level Discovery

The next step was to move discovery from the first linked ELF down to the real
link inputs.

This is now done by the Rust app tool itself:

- `build_fae_rust`

During the discovery build, the tool now runs as a linker wrapper:

1. Cargo/rustc invokes `build_fae_rust` as the linker wrapper.
2. The wrapper sees the real input list:
   - `.o`
   - `.rlib`
3. It scans:
   - standalone ELF objects
   - archive members ending in `.o` inside `.rlib`
4. It records a section plan file:
   - `build/auto-fae-input-sections.txt`
5. It then forwards the original linker command to `rust-lld`.

The section plan format is intentionally simple and line-based. It currently
tracks:

- `rom`
- `got`
- `rom_ram`
- `ram`
- `discard`
- `alloc_abs32`
- `nonalloc_abs32`

### Effect On `rust-fae-helloworld`

Validated with:

```bash
cargo run --manifest-path examples/rust-fae-helloworld/xtask/Cargo.toml -- --elf
cargo run --manifest-path examples/rust-fae-helloworld/xtask/Cargo.toml -- --fae
```

The important change is that the generated linker script is now based on
original input sections, not merged output sections.

Observed result on the generated section plan:

- `.rom`: about 1400 input sections
- `.rom.ram`: 4 precise read-mostly sections
- `.ram`: 1 section

The four projected allocatable ABS32 sections were:

- `.rodata._RNvCs93ENP5pDH3D_19rust_fae_helloworld16___FAE_KEEP_START`
- `.rodata..Lanon.5747a811bc8d31a4a894f6bbb9e04b7d.6`
- `.rodata..Lanon.c017c36f01923a407a998846e38bb572.1`
- `.rodata..Lanon.c017c36f01923a407a998846e38bb572.2`

So the over-approximation is no longer:

- “move all `.rodata*`”

but rather:

- “move only the specific input sections that actually receive `R_ARM_ABS32`”

This is much closer to the intended long-term scheme.

## Boot Trace On The Bootable `.fae`

The new Rust app flow was then exercised with the actual bootable image:

```bash
cargo run --manifest-path examples/rust-fae-helloworld/xtask/Cargo.toml -- bootable mps2-an385
qemu-system-arm -machine mps2-an385 -nographic \
  -semihosting-config enable=on,target=native \
  -kernel examples/rust-fae-helloworld/build/rust_fae_helloworld.fae
```

The image was packaged successfully, but QEMU produced no semihosting output.

To understand where execution stopped, QEMU instruction tracing was enabled on
the generated bootable `.fae`, using:

```bash
qemu-system-arm -machine mps2-an385 -nographic \
  -semihosting-config enable=on,target=native \
  -d in_asm,cpu,guest_errors \
  -D /tmp/qemu-faerust.log \
  -kernel examples/rust-fae-helloworld/build/rust_fae_helloworld.fae
```

### Observed Execution Path

The trace shows the following sequence:

1. `Reset_Handler` in the prepended startup runs normally.
2. The startup copies `.rom.ram`, zeroes `.ram`, and applies the relocation
   table.
3. The startup returns successfully and jumps to Rust `start` at `0x00000dcc`.
4. `start` initializes the allocator and then branches to `fae_main` at
   `0x000002b8`.
5. `fae_main` reaches the first `println!`, enters `Console::write_fmt`, then
   enters `core::fmt::write`.
6. `core::fmt::write` loads its callback pointer from a table at runtime
   address `0x00001060`.
7. That table still contains the pre-relocation function address `0x00000b05`.
8. Control branches to that stale address, falls into the panic path, and then
   recursively panics while trying to print the panic message.
9. The recursive panic loop keeps consuming stack until the stack pointer
   crosses below `0x20000000`.

### Important Addresses Seen In The Trace

Useful checkpoints from the captured trace:

- startup return into Rust runtime:
  - `R15 = 0x00000dcc` (`start`)
- Rust application entry:
  - `R15 = 0x000002b8` (`fae_main`)
- first `println!` callback dispatch:
  - `R15 = 0x00000874` in `core::fmt::write`
  - callback loaded from runtime address `0x00001060`
  - callback value read: `0x00000b05`
- first panic formatting path:
  - `R15 = 0x00000c08`
  - then `R15 = 0x00000cc4`
  - then `R15 = 0x00000838`

The startup trace also shows that the relocation machinery itself computes the
expected relocated values for the moved tables. For example, during relocation
application the startup writes values such as:

- `0x00000dbd`
- `0x00000ced`
- `0x00000cd5`

Those are the correctly rebased counterparts of the stale pre-relocation
function pointers later observed in the formatting tables.

### Conclusion

This experiment is important because it narrows the failure mode:

- the startup stack setup is correct
- the startup relocation pass is running correctly
- the jump `startup -> start -> fae_main` works
- the failure happens when Rust formatting code dereferences a table that is
  still addressed as ROM image data

So moving selected sections into `.rom.ram` is not sufficient by itself.
Those sections must also be *addressed as relocated runtime data* by the code
generated by Rust. In the current setup, at least part of the `core::fmt`
machinery still reaches them through PC-relative ROM addressing, which defeats
the purpose of relocating them into `.rom.ram`.

## Why A Post-Codegen Rewrite Is Now A Serious Option

At this point, the remaining problem is no longer vague:

- the problematic tables have been identified
- the call sites that consume them have been identified
- the startup relocation logic computes the correct rebased values
- the failure comes from the addressing mode chosen by generated Rust code

The practical consequence is that a post-codegen rewrite is no longer a random
hack. It can be scoped as a targeted experiment over a known, small class of
symbols.

### Why This Is Not The First Choice

The natural first choice would have been:

- ask Rust/LLVM to emit those tables directly as true RWPI data

But the investigation so far has not found a simple `rustc` option that forces
compiler-generated metadata tables such as Rust vtable-like structures into a
real SB-relative RWPI addressing scheme.

`-C relocation-model=ropi-rwpi` was already used in the most promising setup,
and that still led to:

- tables physically moved into `.rom.ram`
- but code that continues to reach them through the ROM image

So the issue is not just "data placement". It is "how generated code chooses to
address that data".

### Why A `.s` Rewrite Is Considered Fragile

Rewriting generated assembly is still considered fragile for a few concrete
reasons:

- some PC-relative accesses are valid and should not be rewritten
- Thumb instruction size can change when replacing PC-relative forms with
  base-register-relative forms
- the replacement must preserve or regenerate coherent ELF relocations
- the exact assembly patterns are produced by LLVM and may shift across Rust
  versions

So this is not a good general-purpose foundation for a stable toolchain.

### Why It May Still Be The Only Practical Short-Term Path

Despite those drawbacks, this path is now credible because the scope can be
kept narrow:

1. identify the specific `.Lanon...` / vtable-like symbols that are moved into
   `.rom.ram`
2. find the exact generated references to those symbols in the emitted `.s`
3. rewrite only those references
4. reassemble and relink
5. verify that the resulting accesses use the intended static-base-relative
   scheme and that QEMU reaches semihosting output

So the working conclusion is:

- a broad assembly rewrite remains undesirable
- a targeted rewrite of a known set of Rust-generated metadata references may
  be the only realistic short-term experiment left before modifying LLVM/Rust
  itself
