use core::arch::global_asm;
use core::ptr;

use rust_xipfs_lib::xipfs_rt0_ctx::XipfsRt0Ctx;
use rust_xipfs_lib::xipfs_rt0_ctx_data::XipfsRt0CtxData;

// Calling convention for this Rust startup entrypoint:
//
// - pc: entry at `_start` inside this startup image
// - sp: valid caller stack
// - lr: caller return address
// - r0-r3: forwarded unchanged to the relocated payload entrypoint on the
//   success path
// - r9: start of the RAM window granted to the FAE payload
//
// On success, the runtime calls the payload entrypoint itself with:
//
// - r0-r3 = original caller-provided values
// - r9 = relocated GOT / static base
// - r12 = relocated entrypoint address
//
// If the payload entrypoint returns, control resumes at the caller return
// address in `lr`.
//
// This is the same minimal ABI as `rt0/arm-thumb/rt0-thumb.s`. The caller is
// responsible for inspecting the FAE footer first and must only enter this
// runtime when the RAM window starting at `r9` is large enough for
// `.got + .rom.ram + .ram`.
//
// build_fae rebuilds its embedded startup ELFs from source while it is being
// compiled, so changes here are picked up by rebuilding the host tool.

const CRT0_MAGIC_NUMBER_AND_VERSION: u32 = 0xFACADE12;

const BINARY_FOOTER_RAM_SIZE_OFFSET: isize = -28;
const BINARY_FOOTER_GOT_SIZE_OFFSET: isize = -24;
const BINARY_FOOTER_ROM_SIZE_OFFSET: isize = -20;
const BINARY_FOOTER_ROM_RAM_SIZE_OFFSET: isize = -16;
const BINARY_FOOTER_ENTRYPOINT_OFFSET: isize = -12;
const BINARY_FOOTER_MAGIC_NUMBER_AND_VERSION_OFFSET: isize = -4;

const ERR_INVALID_VERSION: u32 = 1;
const ERR_OUT_OF_BOUNDS: u32 = 3;
const ERR_PTR_IN_ROM: u32 = 4;
const ERR_PTR_IN_GOT: u32 = 5;

#[repr(C)]
struct Rt0Result {
    status: u32,
    next_ram: u32,
    next_nvm: u32,
    entry: u32,
    got: u32,
    ctx_addr: u32,
}

unsafe extern "C" {
    static __metadataOff: u32;
    fn _start();
}

global_asm!(
    r#"
    .section ._start, "ax", %progbits
    .global _start
    .type _start, %function
    .thumb
    .thumb_func
_start:
    push    {{r4, lr}}
    sub     sp, sp, #24
    mov     r4, lr
    ldr     r9, [r0, #4]

    mov     r1, sp
    bl      __rustrt0_thumb_run
    
    ldr     r0, [sp, #0] @status
    cmp     r0, #0
    bne     1f
    ldr     r1, [sp, #4] @next_ram
    ldr     r2, [sp, #8] @next_nvm
    ldr     r3, [sp, #12] @entry
    
    ldr     r9, [sp, #16] @got
    ldr     r0, [sp, #20] @ctx_addr
    push    {{r0}}
    mov     r12, r3 @relocated entry addr
    blx     r3
    mov     lr, r4 
1:
    pop     {{r3}}
    ldr     r3, [r3, #20]
    ldr     r10, [r3, #272]
    ldr     r3, [r3, #8]
    ldr     r3, [r3, #0]
    blx     r3
"#
);

#[inline(always)]
fn round_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

#[inline(always)]
unsafe fn read_u32_at(base: *const u8, offset: isize) -> u32 {
    ptr::read_unaligned(base.offset(offset) as *const u32)
}

#[inline(always)]
unsafe fn memcpy_bootstrap(mut dest: *mut u8, mut src: *const u8, mut n: usize) {
    while n >= 4 {
        ptr::write_unaligned(dest as *mut u32, ptr::read_unaligned(src as *const u32));
        dest = dest.add(4);
        src = src.add(4);
        n -= 4;
    }
    while n > 0 {
        ptr::write(dest, ptr::read(src));
        dest = dest.add(1);
        src = src.add(1);
        n -= 1;
    }
}

#[inline(always)]
unsafe fn resolve_binary_offset(
    mut off: usize,
    rom_sec_addr: usize,
    rel_got_sec_addr: usize,
    rel_rom_ram_sec_addr: usize,
    rel_ram_sec_addr: usize,
    rom_sec_size: usize,
    got_sec_size: usize,
    rom_ram_sec_size: usize,
    ram_sec_size: usize,
) -> Option<usize> {
    if off < rom_sec_size {
        return Some(rom_sec_addr + off);
    }
    off -= rom_sec_size;
    if off < got_sec_size {
        return Some(rel_got_sec_addr + off);
    }
    off -= got_sec_size;
    if off < rom_ram_sec_size {
        return Some(rel_rom_ram_sec_addr + off);
    }
    off -= rom_ram_sec_size;
    if off < ram_sec_size {
        return Some(rel_ram_sec_addr + off);
    }
    None
}

#[unsafe(no_mangle)]
unsafe extern "C" fn __rustrt0_thumb_run(ctx: *const XipfsRt0Ctx, out: *mut Rt0Result) {
    let out = &mut *out;
    let xipfs_rt0_ctx = &*ctx;

    out.status = ERR_INVALID_VERSION;
    out.next_ram = 0;
    out.next_nvm = 0;
    out.entry = 0;
    out.got = 0;
    out.ctx_addr = ctx as u32;
    let metadata_addr =
        (xipfs_rt0_ctx.bin_base as usize) + ((&__metadataOff as *const u32) as usize);
    let start_offset = (_start as *const () as usize) & !1usize;
    let startup_addr = (xipfs_rt0_ctx.bin_base as usize).wrapping_add(start_offset);
    let binary_size = ptr::read_unaligned(metadata_addr as *const u32) as usize;
    let patch_entry_number = ptr::read_unaligned((metadata_addr + 4) as *const u32) as usize;
    let end_of_binary = startup_addr.wrapping_add(binary_size);
    // let end_of_binary_minus_footer = fae_base_bias.wrapping_add(binary_size);

    let magic = read_u32_at(
        end_of_binary as *const u8,
        BINARY_FOOTER_MAGIC_NUMBER_AND_VERSION_OFFSET,
    );
    if magic != CRT0_MAGIC_NUMBER_AND_VERSION {
        return;
    }

    let entry_point_offset =
        read_u32_at(end_of_binary as *const u8, BINARY_FOOTER_ENTRYPOINT_OFFSET) as usize;
    let rom_sec_size =
        read_u32_at(end_of_binary as *const u8, BINARY_FOOTER_ROM_SIZE_OFFSET) as usize;
    let got_sec_size =
        read_u32_at(end_of_binary as *const u8, BINARY_FOOTER_GOT_SIZE_OFFSET) as usize;
    let rom_ram_sec_size = read_u32_at(
        end_of_binary as *const u8,
        BINARY_FOOTER_ROM_RAM_SIZE_OFFSET,
    ) as usize;
    let ram_sec_size =
        read_u32_at(end_of_binary as *const u8, BINARY_FOOTER_RAM_SIZE_OFFSET) as usize;

    if entry_point_offset > rom_sec_size {
        out.status = ERR_OUT_OF_BOUNDS;
        return;
    }

    let payload_padding_addr = metadata_addr + 4 + 4 + patch_entry_number * 4;
    let payload_padding = ptr::read_unaligned(payload_padding_addr as *const u32) as usize;
    let rom_sec_addr = payload_padding_addr + 4 + payload_padding;
    let got_sec_addr = rom_sec_addr + rom_sec_size;
    let rom_ram_sec_addr = got_sec_addr + got_sec_size;
    let entry_point_addr = (rom_sec_addr + entry_point_offset) as u32;

    let rel_got_sec_addr = xipfs_rt0_ctx.ram_start as usize;
    let rel_rom_ram_sec_addr = rel_got_sec_addr + got_sec_size;
    let rel_ram_sec_addr = rel_rom_ram_sec_addr + rom_ram_sec_size;

    memcpy_bootstrap(
        rel_rom_ram_sec_addr as *mut u8,
        rom_ram_sec_addr as *const u8,
        rom_ram_sec_size,
    );

    let mut ram_ptr = rel_ram_sec_addr as *mut u32;
    let mut left = ram_sec_size >> 2;
    while left > 0 {
        ptr::write(ram_ptr, 0);
        ram_ptr = ram_ptr.add(1);
        left -= 1;
    }

    let mut i = 0usize;
    while (i << 2) < got_sec_size {
        let off = ptr::read_unaligned((got_sec_addr as *const u32).add(i)) as usize;
        let addr = match resolve_binary_offset(
            off,
            rom_sec_addr,
            rel_got_sec_addr,
            rel_rom_ram_sec_addr,
            rel_ram_sec_addr,
            rom_sec_size,
            got_sec_size,
            rom_ram_sec_size,
            ram_sec_size,
        ) {
            Some(v) => v,
            None => {
                out.status = ERR_OUT_OF_BOUNDS;
                return;
            }
        };
        ptr::write((rel_got_sec_addr as *mut u32).add(i), addr as u32);
        i += 1;
    }

    let patch_entries = (metadata_addr + 8) as *const u32;
    let mut j = 0usize;
    while j < patch_entry_number {
        let mut ptr_off = ptr::read_unaligned(patch_entries.add(j)) as usize;
        let off = ptr::read_unaligned((rom_sec_addr + ptr_off) as *const u32) as usize;

        if ptr_off < rom_sec_size {
            out.status = ERR_PTR_IN_ROM;
            return;
        }
        ptr_off -= rom_sec_size;
        if ptr_off < got_sec_size {
            out.status = ERR_PTR_IN_GOT;
            return;
        }
        ptr_off -= got_sec_size;

        let ptr_addr = if ptr_off < rom_ram_sec_size {
            rel_rom_ram_sec_addr + ptr_off
        } else {
            ptr_off -= rom_ram_sec_size;
            if ptr_off < ram_sec_size {
                rel_ram_sec_addr + ptr_off
            } else {
                out.status = ERR_OUT_OF_BOUNDS;
                return;
            }
        };

        let addr = match resolve_binary_offset(
            off,
            rom_sec_addr,
            rel_got_sec_addr,
            rel_rom_ram_sec_addr,
            rel_ram_sec_addr,
            rom_sec_size,
            got_sec_size,
            rom_ram_sec_size,
            ram_sec_size,
        ) {
            Some(v) => {
                if v >= rel_got_sec_addr && v < rel_got_sec_addr + got_sec_size {
                    out.status = ERR_PTR_IN_GOT;
                    return;
                }
                v
            }
            None => {
                out.status = ERR_OUT_OF_BOUNDS;
                return;
            }
        };

        ptr::write(ptr_addr as *mut u32, addr as u32);
        j += 1;
    }

    out.status = 0;
    out.next_ram = (rel_ram_sec_addr + ram_sec_size) as u32;
    out.next_nvm = round_up(end_of_binary, 32) as u32;
    out.entry = entry_point_addr;
    out.got = rel_got_sec_addr as u32;
    let ctx_data = xipfs_rt0_ctx.argv as *mut XipfsRt0CtxData;
    (*ctx_data).current_got = rel_got_sec_addr as *const u8;
}
