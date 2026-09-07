# RAM Static Base Layout Example

This example is a minimal Rust `no_std` payload that intentionally creates the
three memory classes used by the Rust FAE layout:

- executable/read-only ROM data in `.rom`
- initialized runtime data in `.rom.ram`
- mutable runtime data in `.ram`

The goal is not to demonstrate an allocator implementation. The `HEAP` array is
only a small mutable static used to force a `.ram` section.

## What The Program Contains

`DESCRIPTOR` is a Rust trait object descriptor. It contains initialized
addresses, so the Rust FAE linker flow places it in `.rom.ram`: it must be
relocated during startup, but after relocation it is conceptually read-only.

`HEAP` is a mutable byte array. It has no initial payload data and is genuinely
writable at runtime, so it belongs in `.ram`.

`start()` writes one byte to `HEAP`, then calls through `DESCRIPTOR`. This
keeps both data classes live in the generated code.

## Why `.rom.ram` And `.ram` Stay Separate

`.rom.ram` and `.ram` are both part of the runtime RAM window, but they do not
have the same role.

`.rom.ram` contains initialized data copied from the FAE image and relocated at
startup. After startup, many of these objects, such as relocated descriptors or
vtables, should not normally be written again.

`.ram` contains runtime-only writable data, such as `.bss`, mutable statics,
and heap storage.

Keeping these as separate ELF sections lets tooling, diagnostics, and future
runtime variants distinguish them precisely.

## MPU Protection Possibility

A custom rt0 could use this split to improve memory protection:

- keep `.got + .rom.ram` writable during startup while relocations are applied
- then mark that initialized area read-only with the MPU
- keep `.ram` writable for the rest of execution

The current rt0 implementations do not perform this MPU transition. Also, not
all ARM microcontroller families can provide an MPU layout that makes this
split practical or portable. For now, the section split is preserved as useful
structure, even when the whole runtime RAM window remains writable.

## Segment Constraint For RWPI

Although `.got`, `.rom.ram`, and `.ram` remain distinct sections, ARM RWPI code
needs them to share one writable ELF `PT_LOAD` segment.

`rust-lld` resolves ARM `BREL/SBREL` relocations using the base of the
`PT_LOAD` segment containing the referenced symbol. XIPRFS startup installs
`r9` as the base of the runtime writable window.

Therefore the generated linker script should produce a layout shaped like:

```text
Segment rx: .rom
Segment rw: .got .rom.ram .ram
```

The sections remain distinct, but the writable runtime window has a single
static base.
