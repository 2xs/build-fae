    .syntax unified
    .cpu cortex-m0plus
    .thumb

    .section ._start, "ax", %progbits
    .global _start
    .type _start, %function
    .thumb_func
_start:
    .include "rt0/arm-thumb/rt0-thumbv6m.s"

    .global _end
_end:
