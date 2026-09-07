    .equ SCB_VTOR,        0xE000ED08
    .equ SYS_WRITE0,      4
    .equ SYS_EXIT,        0x18
    .equ ADP_APP_EXIT,    0x20026
    /* build_fae rebuilds the embedded startup ELFs from source while the host
     * tool is compiled. Rebuild build_fae after editing this startup to pick
     * the changes up in the default embedded flow.
     *
     * Failure mapping:
     * - codes 1, 3, 4, 5 are returned by the shared relocation core
     * - code 2 is a legacy reserved slot kept only for message table stability
     * - code 6 is raised here from HardFault_Handler
     * - code 7 is raised here from MemManage_Handler
     */

    .align 2
    .thumb_func
Reset_Handler:
    cpsid   i
    ldr     r0, =__StackTop
    mov     sp, r0
    mov     r11, sp

    ldr     r0, =__Vectors
    ldr     r1, =SCB_VTOR
    str     r0, [r1]

    ldr     r9, =__fae_ram_start
    bl      .Lfae_rt0_run
    cmp     r0, #0
    bne     .Ldie_common

    mov     sp, r11
    mov     sp, r11
    ldr     r1, =ADP_APP_EXIT
    movs    r0, #SYS_EXIT
    bkpt    0xab

    .thumb_func
HardFault_Handler:
    movs    r0, #6
    b       .Ldie_common

    .thumb_func
MemManage_Handler:
    movs    r0, #7
    b       .Ldie_common

.Ldie_common:
    mov     sp, r11
    push    {r0}
    ldr     r1, =board_name
    bl      .Lwrite0
    pop     {r0}
    subs    r0, #1
    lsls    r0, r0, #2
    adr     r1, .Lmsgtab
    ldr     r1, [r1, r0]
    bl      .Lwrite0
.Lhang:
    nop
    b       .Lhang

.Lwrite0:
    movs    r0, #SYS_WRITE0
    bkpt    0xab
    bx      lr

    .align 2
.Lmsgtab:
    .word .Lerr1, .Lerr2, .Lerr3, .Lerr4, .Lerr5, .Lerr6, .Lerr7
.Lerr1:
    .asciz "invalid file version"
.Lerr2:
    .asciz "not enough ram"
.Lerr3:
    .asciz "out-of-bounds offset"
.Lerr4:
    .asciz "cannot relocate offsets in .rom"
.Lerr5:
    .asciz "cannot relocate offsets in .got"
.Lerr6:
    .asciz "hard fault"
.Lerr7:
    .asciz "memmanage fault"

    .thumb_func
.Lfae_rt0_run:
    .include "rt0/arm-thumb/rt0-thumb.s"

    .global _end
_end:
