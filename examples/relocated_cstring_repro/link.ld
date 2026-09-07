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
        *(.rodata*)
        . = ALIGN(4);
        __rom_end = .;
    }
    __rom_size = __rom_end - __rom_start;

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
        *(.data*)
        . = ALIGN(4);
        __rom_ram_end = .;
    }
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
