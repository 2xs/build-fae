    .syntax unified
    .arch armv7-m
    .thumb

    /*
     * Template bootable FAE startup for a generic Cortex-M board.
     *
     * Copy this directory, then adjust:
     * - .arch
     * - the linker script memory map
     * - the board_name string below
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
    .asciz "replace-with-board-name: " /* Prefix shown in semihosting fault messages. */

    .include "rt0/bootable/generic-cortex-m/boot_rt0.s"
