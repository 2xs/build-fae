# FAE File Format

This document specifies the on-disk FAE format. The default is FAE 1.0,
identified by `0xFAEC0D10`. The transitional `0xFACADE12` format is documented
at the end. Identifiers live in [`fae-registry.md`](fae-registry.md).

## Responsibility Layers

Five concerns must remain separate:

1. The **generic FAE container** identifies format, ISA and ABI and protects the
   complete file with a tail-located CRC.
2. The **call and return contract** defines how byte zero is entered and how it
   returns. Current ARM XiPFS runtimes receive `pc`, `sp`, `lr`, `r0-r3`, and
   static base `r9`.
3. An **RT0** may use private metadata such as binary size, relocations, GOT,
   initialized RAM, BSS and an internal application entrypoint.
4. An **execution ABI** defines host services, component profile, minimum ABI
   version and memory requirements.
5. **Payload internals** such as `.rom`, `.got`, `.rom.ram`, `.ram` and
   relocation tables are choices made by a builder and its matching RT0.

Only the first concern is universal. A generic parser must not assume a GOT,
RT0, `.rom.ram`, stack, relocation table or secondary entrypoint. MPU/PMP
availability is an execution-environment and ABI concern, not an ISA feature.

## Common Encoding

- All words are unsigned 32-bit little-endian values.
- The common footer is anchored at the end of the file.
- Additional-word counts are bounded to `0..=15`.
- Footer sizes and offsets are derived with checked arithmetic.

## FAE 1.0 Footer

| Offset from end | Field |
| ---: | --- |
| `-4` | magic and version, `0xFAEC0D10` |
| `-8` | ISA descriptor |
| `-12` | ABI descriptor |
| `-16` | generic additional-field descriptor |
| `-20` | CRC-32/ISO-HDLC |

The generic descriptor uses bits `7..0` as its additional-word count. Bits
`31..16` are RFU; bits `15..8` are reserved as zero in FAE 1.0. The initial
producer emits descriptor `0x00000000`, hence no generic additional word.

The ISA descriptor contains `family:u8`, `subgroup:u8`, twelve family-specific
inline bits, then a four-bit ISA additional-word count. The ABI descriptor
contains a 28-bit family ID followed by a four-bit ABI additional-word count.

Reading from the fixed words toward decreasing addresses yields generic
additional word zero and remaining generic words, then ISA words, then ABI
words. Their physical low-to-high byte order is consequently reversed.

```text
footer_size = 4 * (5 + extra_word_count
                     + isa_extra_word_count
                     + abi_extra_word_count)
```

The CRC uses CRC-32/ISO-HDLC as implemented by `crc32fast`. It covers every byte
of the file, including padding and all footer words, while treating the four CRC
bytes as zero.

A generic reader first recognizes the final magic, reads the three counts,
derives and bounds-checks the footer size, verifies the CRC, and only then calls
an ABI decoder. Unknown identifiers may be shown raw by an inspector but must
be rejected by operations that need to understand or execute them.

## XiPFS Profile

XiPFS uses ABI family `0xFACADE1`, descriptor `0xFACADE16`, and six ABI words in
logical backward-reading order:

1. zero-initialized writable `.ram` tail size in bytes;
2. GOT image size in bytes;
3. `.rom` size in bytes;
4. initialized `.rom.ram` image size in bytes;
5. application entrypoint value relative to `.rom`;
6. startup-code size in bytes.

With no generic or ISA additional word, its footer is 44 bytes. Physical
low-to-high order is:

```text
startup_code_size, entrypoint_offset, rom_ram_size, rom_size,
got_size, ram_size, crc32, extra_fields_descriptor,
0xFACADE16, isa_descriptor, 0xFAEC0D10
```

The entrypoint may carry the ARM Thumb bit in bit zero. The writable runtime
footprint beginning at `r9` is:

```text
got_size + rom_ram_size + ram_size
```

### XiPFS RT0 Metadata

This layout is private to the current XiPFS builders and RT0, not generic FAE:

```text
[startup code]
[binary_size]
[relocation_count][relocation offsets]
[payload_alignment_size][zero alignment bytes]
[.rom][.got image][.rom.ram image][0xFF padding]
[FAE footer]
```

`binary_size` includes the complete file. Section offsets are derived by:

```text
partition_offset = startup_code_size + 4
                 + 4 + 4 * relocation_count
                 + 4 + payload_alignment_size
rom_offset       = partition_offset
got_offset       = rom_offset + rom_size
rom_ram_offset   = got_offset + got_size
```

At runtime, `.got` starts at `r9`, `.rom.ram` at `r9 + got_size`, and the
zeroed `.ram` tail at `r9 + got_size + rom_ram_size`.

Current RT0 variants only check their expected magic and read these XiPFS
fields. CRC and compatibility checks are loader responsibilities and happen
before entering RT0.

## Oxide SE Profile

Rustlet applications and Security Domains use ABI family `0xAC1DA99` and
descriptor `0xAC1DA992`. Its two ABI words are:

1. component profile and minimum ABI version;
2. packed writable-RAM and stack requirements.

The first word contains a twelve-bit profile marker and a twenty-bit version
`major:u8, minor:u4, patch:u4, revision:u4`:

- `0xA9901000`: Rustlet application requiring 1.0.0.0;
- `0x5DC01000`: Security Domain requiring 1.0.0.0.

Unknown profiles are rejected. Major versions must match; the host
`(minor, patch, revision)` must be at least the requested tuple. RAM and stack
are requirements, not a universal MPU layout. The packed word stores writable
RAM in its upper 16 bits and stack in its lower 16 bits. Both values are counts
of 32-byte units, rounded upward by the producer, for a maximum representable
requirement of 2,097,120 bytes per field.

With no generic or ISA additional word, the Oxide SE footer is 28 bytes. Its
physical low-to-high order is:

```text
packed_memory_requirements, profile_and_minimum_version, crc32,
extra_fields_descriptor, 0xAC1DA992, isa_descriptor, 0xFAEC0D10
```

Section sizes, the internal entrypoint, GOT details and RT0 size are not public
Oxide SE ABI fields. The matching Rustlet RT0 receives them through private
metadata immediately following its code:

```text
[Rustlet RT0]
[binary_size]
[zeroed_ram_size][got_size][rom_size][rom_ram_size][entrypoint]
[relocation_count][relocation offsets]
[payload_alignment_size][zero alignment bytes]
[.rom][.got image][.rom.ram image][0xFF padding]
[Oxide SE FAE footer]
```

Oxide SE accepts this FAE 1.0 Rustlet ABI only; the legacy `0xFACADE12` footer
and the XiPFS ABI profile are not Rustlet execution formats.

The builders expose the public descriptors directly through `--isa`,
`--isa-word`, `--abi`, and `--abi-word`. `--rustlet` and `--securitydomain`
only provide the predefined Oxide SE field values and matching default RT0.
They do not select a different on-disk format. Footer validation belongs to the
caller/loader; the Rustlet RT0 consumes its private relocation metadata only
after that validation has succeeded.

## Legacy `0xFACADE12`

The historical footer actually emitted by this repository is exactly **28
bytes**, not 32, and has no `rfu` word:

| Offset from end | Field |
| ---: | --- |
| `-28` | zero-initialized `.ram` tail size |
| `-24` | GOT image size |
| `-20` | `.rom` size |
| `-16` | `.rom.ram` image size |
| `-12` | entrypoint value relative to `.rom` |
| `-8` | startup-code size |
| `-4` | `0xFACADE12` |

Its early XiPFS RT0 metadata is unchanged. `build_fae --facade12` and
`build_fae_rust --facade12` select this footer and the matching legacy RT0.
Readers recognize both formats during migration.

## Versioning

Adding an ISA or ABI identifier extends the registry without changing
`0xFAEC0D10`. Only a change to common framing or reading rules requires a new
format magic/version.
