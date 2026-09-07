    .thumb

    /*
     * ARMv6-M / Thumb-1 FAE relocation core.
     *
     * This is the Cortex-M0+ compatible equivalent of rt0-thumb.s. It keeps the
     * same minimal ABI but avoids Thumb-2 instructions and high-register
     * addressing forms that are illegal on ARMv6-M.
     */

    .ifdef FAE1_FOOTER
    .equ CRT0_MAGIC,      0xFAEC0D10
    .ifdef OXIDE_SE_ABI
    .equ FAE_FOOTER_SIZE, 28
    .else
    .equ FAE_FOOTER_SIZE, 44
    .endif
    .else
    .equ CRT0_MAGIC,      0xFACADE12
    .equ FAE_FOOTER_SIZE, 28
    .endif

    .equ L_RAM_SIZE,      0
    .equ L_GOT_SIZE,      4
    .equ L_ROM_SIZE,      8
    .equ L_ROMRAM_SIZE,   12
    .equ L_ENTRY_OFFSET,  16
    .equ L_ROM_BASE,      20
    .equ L_PATCH_COUNT,   24
    .equ L_PATCH_CURSOR,  28
    .equ L_ENTRY_ABS,     32
    .equ L_PATCH_SLOT,    36
    .equ L_FRAME_SIZE,    40

    push    {r0-r3, lr}
    sub     sp, #L_FRAME_SIZE

    adr     r4, _end                   @ r4 = metadata base
    mov     r5, pc
    subs    r5, #10                    @ r5 = FAE image base
    ldr     r0, [r4]                   @ r0 = binary size
    mov     r6, r5
    add     r6, r0
    subs    r6, #FAE_FOOTER_SIZE       @ r6 = footer base

    movs    r0, #1
    .ifdef FAE1_FOOTER
    .ifdef OXIDE_SE_ABI
    ldr     r1, [r6, #24]              @ magic/version
    .else
    ldr     r1, [r6, #40]              @ magic/version
    .endif
    .else
    ldr     r1, [r6, #24]              @ magic/version
    .endif
    ldr     r2, .Lfae_v6m_magic
    cmp     r1, r2
    beq     .Lfae_v6m_magic_ok
    b       .Lfae_v6m_return
.Lfae_v6m_magic_ok:

    .ifdef FAE1_FOOTER
    .ifdef OXIDE_SE_ABI
    ldr     r0, [r4, #4]
    str     r0, [sp, #L_RAM_SIZE]
    ldr     r0, [r4, #8]
    str     r0, [sp, #L_GOT_SIZE]
    ldr     r0, [r4, #12]
    str     r0, [sp, #L_ROM_SIZE]
    ldr     r0, [r4, #16]
    str     r0, [sp, #L_ROMRAM_SIZE]
    ldr     r0, [r4, #20]
    str     r0, [sp, #L_ENTRY_OFFSET]
    .else
    ldr     r0, [r6, #20]
    str     r0, [sp, #L_RAM_SIZE]
    ldr     r0, [r6, #16]
    str     r0, [sp, #L_GOT_SIZE]
    ldr     r0, [r6, #12]
    str     r0, [sp, #L_ROM_SIZE]
    ldr     r0, [r6, #8]
    str     r0, [sp, #L_ROMRAM_SIZE]
    ldr     r0, [r6, #4]
    str     r0, [sp, #L_ENTRY_OFFSET]
    .endif
    .else
    ldr     r0, [r6, #0]
    str     r0, [sp, #L_RAM_SIZE]
    ldr     r0, [r6, #4]
    str     r0, [sp, #L_GOT_SIZE]
    ldr     r0, [r6, #8]
    str     r0, [sp, #L_ROM_SIZE]
    ldr     r0, [r6, #12]
    str     r0, [sp, #L_ROMRAM_SIZE]
    ldr     r0, [r6, #16]
    str     r0, [sp, #L_ENTRY_OFFSET]
    .endif

    .ifdef OXIDE_SE_ABI
    ldr     r0, [r4, #24]              @ patch count
    .else
    ldr     r0, [r4, #4]               @ patch count
    .endif
    str     r0, [sp, #L_PATCH_COUNT]
    lsls    r0, r0, #2
    mov     r7, r4
    .ifdef OXIDE_SE_ABI
    adds    r7, #28
    .else
    adds    r7, #8
    .endif
    add     r7, r0                     @ r7 = payload padding word
    ldr     r0, [r7]
    adds    r7, #4
    add     r7, r0                     @ r7 = .rom base
    str     r7, [sp, #L_ROM_BASE]

    movs    r0, #3
    ldr     r1, [sp, #L_ENTRY_OFFSET]
    ldr     r2, [sp, #L_ROM_SIZE]
    cmp     r1, r2
    blo     .Lfae_v6m_entry_ok
    b       .Lfae_v6m_return
.Lfae_v6m_entry_ok:
    mov     r0, r7
    add     r0, r1
    str     r0, [sp, #L_ENTRY_ABS]

    @ Copy .rom.ram from NVM to RAM: dst = r9 + got, src = rom + rom_size + got.
    mov     r0, r9
    ldr     r3, [sp, #L_GOT_SIZE]
    add     r0, r3
    mov     r1, r7
    ldr     r2, [sp, #L_ROM_SIZE]
    add     r1, r2
    add     r1, r3
    ldr     r6, [sp, #L_ROMRAM_SIZE]
    lsrs    r6, r6, #2
    beq     .Lfae_v6m_zero_ram
.Lfae_v6m_copy_w:
    ldr     r2, [r1]
    str     r2, [r0]
    adds    r1, #4
    adds    r0, #4
    subs    r6, #1
    bne     .Lfae_v6m_copy_w

.Lfae_v6m_zero_ram:
    ldr     r1, [sp, #L_RAM_SIZE]
    lsrs    r1, r1, #2
    beq     .Lfae_v6m_got_init
    movs    r6, #0
.Lfae_v6m_zero_loop:
    str     r6, [r0]
    adds    r0, #4
    subs    r1, #1
    bne     .Lfae_v6m_zero_loop

.Lfae_v6m_got_init:
    ldr     r3, [sp, #L_GOT_SIZE]
    cmp     r3, #0
    beq     .Lfae_v6m_patch_init
    movs    r6, #0                     @ GOT offset
.Lfae_v6m_got_loop:
    ldr     r1, [sp, #L_ROM_BASE]
    ldr     r2, [sp, #L_ROM_SIZE]
    add     r1, r2
    add     r1, r6
    ldr     r0, [r1]
    bl      .Lfae_v6m_resolve
    mov     r1, r9
    add     r1, r6
    str     r0, [r1]
    adds    r6, #4
    cmp     r6, r3
    blo     .Lfae_v6m_got_loop

.Lfae_v6m_patch_init:
    mov     r0, r4
    .ifdef OXIDE_SE_ABI
    adds    r0, #28
    .else
    adds    r0, #8
    .endif
    str     r0, [sp, #L_PATCH_CURSOR]
    ldr     r1, [sp, #L_PATCH_COUNT]
    cmp     r1, #0
    beq     .Lfae_v6m_ok

.Lfae_v6m_patch_loop:
    ldr     r0, [sp, #L_PATCH_CURSOR]
    ldr     r6, [r0]                   @ patched word offset in .rom
    adds    r0, #4
    str     r0, [sp, #L_PATCH_CURSOR]

    ldr     r0, [sp, #L_ROM_BASE]
    add     r0, r6
    ldr     r0, [r0]                   @ raw pointed value

    movs    r2, #4
    ldr     r3, [sp, #L_ROM_SIZE]
    cmp     r6, r3
    blo     .Lfae_v6m_return_code_r2
    subs    r6, r6, r3
    movs    r2, #5
    ldr     r3, [sp, #L_GOT_SIZE]
    cmp     r6, r3
    blo     .Lfae_v6m_return_code_r2
    subs    r6, r6, r3
    ldr     r3, [sp, #L_ROMRAM_SIZE]
    cmp     r6, r3
    blo     .Lfae_v6m_patch_slot_romram
    movs    r2, #3
    subs    r6, r6, r3
    ldr     r3, [sp, #L_RAM_SIZE]
    cmp     r6, r3
    bhs     .Lfae_v6m_return_code_r2
    mov     r2, r9
    ldr     r3, [sp, #L_GOT_SIZE]
    add     r2, r3
    ldr     r3, [sp, #L_ROMRAM_SIZE]
    add     r2, r3
    add     r2, r6
    b       .Lfae_v6m_patch_slot_ok
.Lfae_v6m_patch_slot_romram:
    mov     r2, r9
    ldr     r3, [sp, #L_GOT_SIZE]
    add     r2, r3
    add     r2, r6
.Lfae_v6m_patch_slot_ok:
    str     r2, [sp, #L_PATCH_SLOT]

    bl      .Lfae_v6m_resolve
    movs    r2, #5
    cmp     r0, r9
    blo     .Lfae_v6m_patch_store
    mov     r3, r9
    ldr     r6, [sp, #L_GOT_SIZE]
    add     r3, r6
    cmp     r0, r3
    blo     .Lfae_v6m_return_code_r2
.Lfae_v6m_patch_store:
    ldr     r2, [sp, #L_PATCH_SLOT]
    str     r0, [r2]
    ldr     r1, [sp, #L_PATCH_COUNT]
    subs    r1, #1
    str     r1, [sp, #L_PATCH_COUNT]
    bne     .Lfae_v6m_patch_loop

.Lfae_v6m_ok:
    ldr     r6, [sp, #L_ENTRY_ABS]
    add     sp, #L_FRAME_SIZE
    pop     {r0-r3, r7}
    push    {r7}
    blx     r6
    pop     {r7}
    bx      r7

.Lfae_v6m_resolve:
    ldr     r3, [sp, #L_ROM_SIZE]
    cmp     r0, r3
    blo     .Lfae_v6m_res_rom
    subs    r0, r0, r3
    ldr     r3, [sp, #L_GOT_SIZE]
    cmp     r0, r3
    blo     .Lfae_v6m_res_got
    subs    r0, r0, r3
    ldr     r3, [sp, #L_ROMRAM_SIZE]
    cmp     r0, r3
    blo     .Lfae_v6m_res_romram
    subs    r0, r0, r3
    ldr     r3, [sp, #L_RAM_SIZE]
    cmp     r0, r3
    bhs     .Lfae_v6m_die2
    mov     r1, r9
    ldr     r3, [sp, #L_GOT_SIZE]
    add     r1, r3
    ldr     r3, [sp, #L_ROMRAM_SIZE]
    add     r1, r3
    add     r0, r1
    bx      lr
.Lfae_v6m_res_romram:
    mov     r1, r9
    ldr     r3, [sp, #L_GOT_SIZE]
    add     r1, r3
    add     r0, r1
    bx      lr
.Lfae_v6m_res_got:
    mov     r1, r9
    add     r0, r1
    bx      lr
.Lfae_v6m_res_rom:
    ldr     r1, [sp, #L_ROM_BASE]
    add     r0, r1
    bx      lr

.Lfae_v6m_die2:
    movs    r0, #3
    b       .Lfae_v6m_return

.Lfae_v6m_return_code_r2:
    mov     r0, r2
.Lfae_v6m_return:
    add     sp, #L_FRAME_SIZE
    add     sp, #16
    pop     {pc}

    .align 2
.Lfae_v6m_magic:
    .word CRT0_MAGIC
