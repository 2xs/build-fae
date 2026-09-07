    .syntax unified
    .arch armv8-m.main
    .thumb

    /*
     * Bootable FAE startup for Raspberry Pi Pico 2 / RP2350.
     *
     * Current assumptions for this first target:
     * - single core bring-up
     * - firmware-style boot from XIP flash
     * - no TrustZone split yet
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
    .asciz "pico2-rp2350: "

    .include "rt0/bootable/generic-cortex-m/boot_rt0.s"
