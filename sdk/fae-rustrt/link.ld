OUTPUT_FORMAT(
    "elf32-littlearm",
    "elf32-littlearm",
    "elf32-littlearm"
)
OUTPUT_ARCH(arm)
ENTRY(start)

SECTIONS
{
    .rom :
    {
        . = ALIGN(4);
        __rom_start = .;
        *(.text*)
        *(.rodata)
        *(.rodata.str*)
        *(.rodata.cst*)
        . = ALIGN(4);
    }

    .ARM.exidx :
    {
        . = ALIGN(4);
        __exidx_start = .;
        *(.ARM.exidx*)
        __exidx_end = .;
        . = ALIGN(4);
    }
    __rom_size = SIZEOF(.rom) + SIZEOF(.ARM.exidx);
    __rom_end = __rom_start + __rom_size;

    .got :
    {
        . = ALIGN(4);
        __got_start = .;
        *(.got*)
        . = ALIGN(4);
        __got_end = .;
    }
    __got_size = __got_end - __got_start;

    .rom.ram :
    {
        . = ALIGN(4);
        __rom_ram_start = .;
        /* Experiment: move relocatable read-mostly Rust tables into the
         * ROM-backed writable block so their ABS32 entries can be fixed
         * during relocation like normal .data. */
        __rom_ram_ro_start = .;
        *(.data.rel.ro)
        *(.data.rel.ro.*)
        *(.rodata.*)
        . = ALIGN(4);
        __rom_ram_ro_end = .;

        __rom_ram_rw_start = .;
        *(.data*)
        . = ALIGN(4);
        __rom_ram_rw_end = .;
        __rom_ram_end = .;
    }
    __rom_ram_ro_size = __rom_ram_ro_end - __rom_ram_ro_start;
    __rom_ram_rw_size = __rom_ram_rw_end - __rom_ram_rw_start;
    __rom_ram_size = __rom_ram_end - __rom_ram_start;

    .ram (NOLOAD) :
    {
        . = ALIGN(4);
        __ram_start = .;
        *(.bss* COMMON)
        . = ALIGN(4);
        __ram_end = .;
    }
    __ram_size = __ram_end - __ram_start;
}
