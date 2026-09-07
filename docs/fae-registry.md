# FAE Identifier Registry

This is the authoritative identifier registry mirrored by
`crates/fae-core/src/fae1.rs`. It intentionally lists only current assignments.

## ISA Families

| ID | Family |
| ---: | --- |
| `0x01` | ARM |
| `0x02` | RISC-V |
| `0x03` | Intel, details RFU |
| `0xFC`, `0xFD` | private or experimental |

## ARM Subgroups

| ID | Subgroup |
| ---: | --- |
| `0x01` | A32 used by the current ARM-state RT0 |
| `0x02` | Thumb-2 used by current Cortex-M3/M4 targets |
| `0x03` | Thumb ARMv6-M used by Cortex-M0+ |

Their twelve inline bits are initially zero. MPU capability is not encoded.

## RISC-V Text

Optional additional words contain a canonical lowercase ASCII ISA string. It
begins with `rv32` or `rv64`, is NUL-terminated, zero-padded to a whole word,
and limited to fifteen words. PMP capability is not encoded.

## ABI Families

| ID | Family |
| ---: | --- |
| `0xFACADE1` | XiPFS |
| `0xAC1DA99` | Oxide SE |

## Oxide SE Profiles

| Marker | Profile |
| ---: | --- |
| `0xA99` | Rustlet application |
| `0x5DC` | Rustlet Security Domain |

New assignments extend this registry without changing the FAE 1.0 magic.
