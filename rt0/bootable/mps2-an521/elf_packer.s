    .syntax unified
    .arch armv8-m.main
    .thumb

    /*
     * ELF wrapper for booting a raw FAE on QEMU mps2-an521.
     *
     * QEMU 10.2.1 on mps2-an521 does not appear to initialize MSP/PC from the
     * FAE vector table when the FAE is loaded as a raw binary. This wrapper
     * embeds the FAE unchanged, reads MSP and Reset_Handler from its vector,
     * programs VTOR, and jumps into the firmware entry point.
     */

    .equ SCB_VTOR, 0xE000ED08

    .section .text._start, "ax", %progbits
    .global _start
    .type _start, %function
    .thumb_func
_start:
    ldr     r0, =__fae_base
    ldr     r1, [r0]
    ldr     r2, [r0, #4]

    ldr     r3, =SCB_VTOR
    str     r0, [r3]

    msr     msp, r1
    bx      r2

    .section .fae_blob, "a", %progbits
    .global __fae_base
__fae_base:
    .incbin FAE_PATH
    .global __fae_limit
__fae_limit:
