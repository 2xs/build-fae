    .thumb

    /*
     * Shared Thumb FAE relocation core.
     *
     * Calling convention for the relocation core itself:
     *
     * Required inputs:
     * - pc  : execution starts at `_start` inside this very image.
     *         The code uses PC-relative addressing against `_start` and `_end`
     *         to find the embedded metadata and the appended FAE payload.
     * - sp  : valid caller stack. The core preserves it across the relocation
     *         process and across the payload call.
     * - lr  : caller return address. This core is intended to be entered via
     *         `bl`; startup failures return to the caller. On success, the core
     *         calls the payload entrypoint itself and only returns if that
     *         payload entrypoint returns.
     * - r0-r3 : forwarded unchanged to the relocated payload entrypoint on the
     *         success path.
     * - r9  : start of the RAM window granted to the FAE payload.
     *         After successful relocation, this same register becomes the
     *         relocated GOT base / app static base expected by generated code.
     *
     * RAM sizing policy:
     * - this core does not verify that the granted RAM window is large enough
     *   for `.got + .rom.ram + .ram`
     * - the caller must inspect the FAE footer first and must only call this
     *   runtime when the RAM window starting at `r9` is large enough
     * - otherwise the relocation writes will overflow the caller-provided RAM
     *   region
     *
     * Scratch / preserved registers:
     * - r10 : scratch register only. It is used for footer and copy/patch
     *         address calculations. It is not an input to the ABI.
     * - r11 : scratch register only. It is used as the relocation patch table
     *         cursor. It is not an input to the ABI.
     *
     * Information discovered by the core from its own in-image data:
     * By construction, the startup image is immediately followed by the FAE
     * metadata. The first word after `_end` is the full FAE binary size:
     * - at `_end + 0` : `binary_size`
     * - at `_end + 4` : relocation patch count
     * - at `_end + 8` : relocation patch table
     * `binary_size` lets the core find the FAE footer at the end of the image:
     * - at `FAE end - 28 .. FAE end` : footer words
     *   `.ram size`, `.got size`, `.rom size`, `.rom.ram size`,
     *   `entrypoint offset`, startup size, magic/version
     *
     * State on successful call into the payload:
     * - r9  : relocated GOT base / app static base
     * - r12 : relocated entrypoint address used for the call
     *
     * If the payload entrypoint returns, control resumes at the caller return
     * address in `lr`.
     *
     * Output on failure:
     * - r0  : startup rejection code returned to the caller before the payload
     *         entrypoint is called
     *         1 = invalid file version
     *         3 = out-of-bounds offset
     *         4 = cannot relocate offsets in .rom
     *         5 = cannot relocate offsets in .got
     *         2 remains a legacy reserved slot and is not emitted here
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

    push    {r0-r3, lr}
    adr     r8, _end                   @ r8 = metadata base
    adr     r7, _start-FAE_FOOTER_SIZE @ r7 = FAE base bias
    bic     r7, r7, #1                 @ clear Thumb bit from _start-derived address arithmetic
    ldr     r0, [r8]
    add     r10, r7, r0                @ r10 = footer base

    movs    r0, #1
    mov     r1, r10
    .ifdef FAE1_FOOTER
    .ifdef OXIDE_SE_ABI
    ldr     r2, [r8, #4]               @ private .ram size
    ldr     r3, [r8, #8]               @ private .got size
    ldr     r4, [r8, #12]              @ private .rom size
    ldr     r5, [r8, #16]              @ private .rom.ram size
    ldr     r6, [r8, #20]              @ private entrypoint
    ldr     r1, [r1, #24]              @ magic/version
    .else
    ldr     r6, [r1, #4]               @ entrypoint
    ldr     r5, [r1, #8]               @ .rom.ram
    ldr     r4, [r1, #12]              @ .rom
    ldr     r3, [r1, #16]              @ .got
    ldr     r2, [r1, #20]              @ .ram
    ldr     r1, [r1, #40]              @ magic/version
    .endif
    .else
    ldmia   r1!, {r2-r6}               @ r2=.ram r3=.got r4=.rom r5=.rom.ram r6=entry
    ldr     r1, [r1, #4]
    .endif
    ldr     r10, .Lfae_magic
    cmp     r1, r10
    bne     .Lfae_return

    .ifdef OXIDE_SE_ABI
    ldr     r10, [r8, #24]             @ r10 = patch count
    add.w   r7, r8, r10, lsl #2
    adds    r7, #28                     @ r7 = payload padding word
    .else
    ldr     r10, [r8, #4]              @ r10 = patch count
    add.w   r7, r8, r10, lsl #2
    adds    r7, #8                     @ r7 = payload padding word
    .endif
    ldr     r0, [r7]                   @ r0 = payload padding size
    adds    r7, #4
    add     r7, r0                     @ r7 = .rom base

    movs    r0, #3
    cmp     r6, r4
    bhs     .Lfae_return
    mov     r12, r6
    add     r12, r7                    @ r12 = relocated entrypoint

    add     r0, r9, r3                 @ r0 = .rom.ram dst
    add     r1, r7, r4
    add     r1, r1, r3                 @ r1 = .rom.ram src
    lsrs    r6, r5, #2                 @ r6 = .rom.ram word count
    beq     .Lfae_zero_ram
.Lfae_copy_w:
    ldr     r10, [r1], #4
    str     r10, [r0], #4
    subs    r6, #1
    bne     .Lfae_copy_w

.Lfae_zero_ram:
    lsrs    r1, r2, #2                 @ r1 = .ram word count
    cbz     r1, .Lfae_got_init
    movs    r6, #0
.Lfae_zero_loop:
    str     r6, [r0], #4
    subs    r1, #1
    bne     .Lfae_zero_loop

.Lfae_got_init:
    movs    r6, #0                     @ r6 = GOT offset
    cbz     r3, .Lfae_patch_init
.Lfae_got_loop:
    add     r1, r7, r4                 @ r1 = GOT image in NVM
    ldr     r0, [r1, r6]
    bl      .Lfae_resolve
    str     r0, [r9, r6]
    adds    r6, #4
    cmp     r6, r3
    blo     .Lfae_got_loop

.Lfae_patch_init:
    adr     r8, _end
    mov     r0, r8
    .ifdef OXIDE_SE_ABI
    ldr     r1, [r0, #24]              @ r1 = patch count
    adds    r11, r0, #28               @ r11 = patch table cursor
    .else
    ldr     r1, [r0, #4]               @ r1 = patch count
    adds    r11, r0, #8                @ r11 = patch table cursor
    .endif
    cbz     r1, .Lfae_ok

.Lfae_patch_loop:
    ldr     r6, [r11], #4              @ r6 = patched word offset in .rom
    ldr     r8, [r7, r6]               @ r8 = raw pointed value

    movs    r0, #4
    cmp     r6, r4
    blo     .Lfae_return
    subs    r6, r6, r4
    movs    r0, #5
    cmp     r6, r3
    blo     .Lfae_return
    subs    r6, r6, r3
    cmp     r6, r5
    blo     .Lfae_ptr_rom_ram
    movs    r0, #3
    subs    r6, r6, r5
    cmp     r6, r2
    bhs     .Lfae_return
    add     r10, r9, r3
    add     r10, r10, r5
    add     r10, r10, r6               @ r10 = patch slot in .ram
    b       .Lfae_ptr_ok
.Lfae_ptr_rom_ram:
    add     r10, r9, r3
    add     r10, r10, r6               @ r10 = patch slot in .rom.ram
.Lfae_ptr_ok:

    mov     r0, r8
    mov     r8, r1                     @ preserve patch count across resolve
    bl      .Lfae_resolve
    mov     r1, r8
    mov     r6, r0                     @ r6 = resolved pointer
    movs    r0, #5
    cmp     r6, r9
    blo     .Lfae_patch_store
    add     r8, r9, r3                 @ r8 = GOT end
    cmp     r6, r8
    blo     .Lfae_return
.Lfae_patch_store:
    str     r6, [r10]
    subs    r1, #1
    bne     .Lfae_patch_loop

.Lfae_ok:
    pop     {r0-r3, lr}
    push    {lr}
    blx     r12
    pop     {lr}
    b       .Lfae_return_after_call

.Lfae_resolve:
    cmp     r0, r4
    blo     .Lfae_res_rom
    subs    r0, r0, r4
    cmp     r0, r3
    blo     .Lfae_res_got
    subs    r0, r0, r3
    cmp     r0, r5
    blo     .Lfae_res_rom_ram
    subs    r0, r0, r5
    cmp     r0, r2
    bhs     .Lfae_die2
    add     r1, r9, r3
    add     r1, r1, r5                 @ r1 = .ram base
    add     r0, r0, r1
    bx      lr
.Lfae_res_rom_ram:
    add     r1, r9, r3                 @ r1 = .rom.ram base
    add     r0, r0, r1
    bx      lr
.Lfae_res_got:
    add     r0, r0, r9
    bx      lr
.Lfae_res_rom:
    add     r0, r0, r7
    bx      lr

.Lfae_die2:
    movs    r0, #3
    b       .Lfae_return

.Lfae_return:
    add     sp, #16
    pop     {pc}

.Lfae_return_after_call:
    bx      lr

    .ltorg
    .align 2
.Lfae_magic:
    .word CRT0_MAGIC
