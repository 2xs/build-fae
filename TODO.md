# TODO

## FAE 1.0 Extensible Footer

The next FAE footer must describe the executable ISA independently from the
runtime ABI. It must remain extensible without assigning an exhaustive CPU or
ABI catalogue in advance.

All footer words use the same little-endian encoding as the current
`0xFACADE12` format. The footer is addressed backwards from the end of the FAE
file. Its fixed words are:

| Offset from end | Field |
| --- | --- |
| `-4` | FAE format magic and version: `0xFAEC0D10` for FAE 1.0 |
| `-8` | ISA descriptor |
| `-12` | ABI descriptor |
| `-16` | generic additional-field descriptor |
| `-20` | CRC-32 of the complete FAE file |

The ISA descriptor has this layout:

```text
31             24 23             16 15           4 3          0
+----------------+-----------------+---------------+------------+
| ISA family ID  | ISA subgroup ID | inline fields | extra words|
+----------------+-----------------+---------------+------------+
       8 bits            8 bits          12 bits       4 bits
```

Initial ISA family assignments are:

- `0x01`: ARM;
- `0x02`: RISC-V;
- `0x03`: Intel, with subgroups and inline fields reserved for now;
- `0xFC` and `0xFD`: private or experimental ISA families.

Each ISA family defines its own subgroup IDs and the meaning of its twelve
inline bits. Only the ARM Thumb subgroups required by the current tooling need
to be assigned initially. RISC-V may use its additional words for the canonical
ASCII ISA string. Such a string is NUL-terminated and padded with zero bytes to
a complete 32-bit word. New assignments extend the ISA registry; they do not
change the FAE format version.

The ABI descriptor has this layout:

```text
31                                              4 3          0
+------------------------------------------------+------------+
|                 ABI family ID                  | extra words|
+------------------------------------------------+------------+
                       28 bits                        4 bits
```

The generic additional-field descriptor uses its low eight bits as its word
count; bits 31..16 are RFU and bits 15..8 are reserved as zero in FAE 1.0. The initial producer emits the descriptor value
`0x00000000`, hence no generic additional word. The four low bits of the ISA and ABI descriptors
encode between zero and fifteen additional words. Reading backwards from the
fixed footer, generic additional word zero comes first, followed by ISA words
and then ABI words. The footer size is therefore:

```text
footer_size = 4 * (5 + extra_word_count + isa_extra_word_count + abi_extra_word_count)
```

The CRC word is a format-level field rather than an ABI-specific field, so a
generic reader can validate an image before interpreting either descriptor.
Use CRC-32/ISO-HDLC (the CRC implemented by `crc32fast`), store it as a
little-endian word, and compute it over the complete FAE byte sequence,
including padding and every footer word, while treating the four CRC bytes as
zero. This validates the payload and the footer received by a loader.

### XiPFS ABI Profile

The XiPFS ABI family ID is `0xFACADE1`. Its initial descriptor is
`0xFACADE16`: six additional words preserve every field emitted by the current
`0xFACADE12` implementation, in this order when read backwards:

1. writable zero-initialized RAM size;
2. GOT image size;
3. ROM size;
4. ROM-to-RAM initialized-data size;
5. entrypoint offset inside ROM;
6. startup-code size.

The existing in-image `binary_size`, relocation count/table, and payload
alignment metadata remain part of the XiPFS startup representation. They are
not reclassified as generic FAE footer fields in this first migration.

### Oxide SE ABI Profile

Rustlet applications and Rustlet Security Domains share ABI family ID
`0xAC1DA99`. Their initial descriptor is `0xAC1DA992`, with two additional
words in this order when read backwards:

1. ABI profile and minimum ABI version;
2. writable RAM and stack requirements, packed as two unsigned 16-bit counts
   of 32-byte units.

The ABI profile/version word uses its high twelve bits as a profile marker and
its low twenty bits as `major:u8, minor:u4, patch:u4, revision:u4`:

- `0xA9901000`: Rustlet application requiring ABI 1.0.0.0;
- `0x5DC01000`: Rustlet Security Domain requiring ABI 1.0.0.0.

A host must reject an unknown profile marker. For a known profile, the major
version must match and the host-provided `(minor, patch, revision)` tuple must
be greater than or equal to the minimum tuple requested by the image.

The writable-RAM and stack fields deliberately describe runtime requirements;
they do not prescribe a particular MPU layout. The Oxide SE loader remains
responsible for translating these requirements into target-specific isolated
memory windows. RT0-private section sizes and the internal entrypoint are stored
after the Rustlet RT0 and are not exposed as public ABI fields.

## FAE 1.0 Implementation Plan

- [x] Add shared typed structures and checked encoders/decoders for the FAE 1.0
  fixed trailer, ISA descriptor, ABI descriptor, and bounded additional words.
- [x] Define the initial ARM Thumb subgroup IDs and inline feature bits used by
  the currently supported Rust and C build targets.
- [x] Define canonical encoding and validation of the optional RISC-V ISA text
  carried by ISA additional words.
- [x] Implement CRC-32/ISO-HDLC generation and validation over the complete FAE
  image with the stored CRC field treated as zero during computation.
- [x] Implement the XiPFS `0xFACADE16` ABI profile and verify that it preserves
  every field actually emitted by the current 28-byte `0xFACADE12` footer.
- [x] Correct `docs/fae-format.md`, which still describes an obsolete 32-byte
  footer with an `rfu` word while the current builder emits seven words and 28
  bytes.
- [x] Implement the compact Oxide SE `0xAC1DA992` ABI profile with application
  `0xA9901000` and Security Domain `0x5DC01000` profile/version words.
- [x] Teach `build_fae`, `build_fae_rust`, and their shared libraries to emit
  the FAE 1.0 footer by default.
- [x] Expose explicit ISA and ABI descriptor/word options, with `--rustlet`
  and `--securitydomain` implemented as convenience profiles over the same
  footer builder and with an explicit RT0 selection taking precedence.
- [x] Add `--facade12` to the relevant builders so they can explicitly emit the
  legacy `0xFACADE12` footer during the migration period.
- [x] Keep readers and inspection tools able to decode both `0xFACADE12` and
  `0xFAEC0D10`, while reporting the selected format and decoded ISA/ABI
  profiles explicitly.
- [ ] Harden each in-tree RT0's private relocation-metadata arithmetic before
  dereferencing payload data. Public footer, ISA and ABI validation belongs to
  the caller/loader and must not be duplicated in the RT0.
- [x] Update Oxide SE final `LOAD` validation to check CRC, ISA compatibility,
  Rustlet versus Security Domain ABI profile, and minimum ABI version before
  publishing a persistent package.
- [ ] Add malformed-footer tests for truncated extension arrays, counts above
  available data, integer overflow, unknown profiles, incompatible versions,
  corrupted payloads, corrupted footer fields, and CRC mismatch.
- [ ] Add equivalence tests proving that legacy `--facade12` and FAE 1.0 XiPFS
  images describe the same startup, section, entrypoint, and RAM requirements.
- [x] Document the allocation policy for ISA families, ISA subgroups, inline
  feature bits, ABI family IDs, ABI profile markers, and private/experimental
  ranges in one authoritative registry.

## Later Work

- [ ] Manage stack size.
- [ ] Run FAE images on QEMU.
- [ ] Clarify the syscall table.
- [ ] Check FAE integrity in `rt0` beyond the FAE 1.0 CRC work above.
- [ ] Test `rt0-arm`.
- [ ] Support the FAE format for `riscv32-unknown-none-ropi-rwpi`.
- [ ] Document the current stack contract explicitly: the stack is provided by
  the loader/runtime, not by the legacy FAE file format.
- [ ] Provide starter SDKs that export `start(ctx)` and call user `main()`, so
  `build_fae` can stay a packer rather than becoming a linker.
- [ ] Fix `cargo check` for the Rust SDK flow once the workspace reorganization
  is complete.
- [ ] Clean the remaining documentation under `research/`.
- [ ] Remove remaining absolute paths from repository documentation and
  generated references where appropriate.
