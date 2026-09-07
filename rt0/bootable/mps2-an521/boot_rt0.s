    .syntax unified
    .arch armv8-m.main
    .thumb

    /*
     * Bootable FAE startup for QEMU mps2-an521.
     *
     * This startup blob is itself the first bytes of the final FAE file, so
     * the generated .fae is both a valid FAE and a raw Cortex-M firmware
     * image.
     */

    .section ._start, "ax", %progbits
    .align 2

    .global __Vectors
    .type __Vectors, %object
__Vectors:
    .global _start
    .thumb_set _start, __Vectors
    .word   __StackTop
    .word   Reset_Handler
    .word   0
    .word   HardFault_Handler
    .word   MemManage_Handler

board_name:
    .asciz "mps2-an521: "

    .include "rt0/bootable/generic-cortex-m/boot_rt0.s"
