ENTRY(_start)

__STACK_SIZE_CPU0 = 0x00004000;

MEMORY
{
  FLASH (rx) : ORIGIN = 0x08000000, LENGTH = 1024K
  RAM   (rwx): ORIGIN = 0x20000000, LENGTH = 128K
}

SECTIONS
{
  . = ORIGIN(FLASH);

  ._start : ALIGN(4)
  {
    KEEP(*("._start"))
    . = ALIGN(4);
  } > FLASH

  __metadataOff = .;
  ASSERT(__metadataOff == _end, "_end must stay at end of ._start")

  /DISCARD/ :
  {
    *(.ARM.exidx*)
    *(.eh_frame*)
    *(.comment)
    *(.note*)
    *(.ARM.attributes)
  }
}

__StackTop      = ORIGIN(RAM) + LENGTH(RAM);
__StackLimit    = __StackTop - __STACK_SIZE_CPU0;
__fae_ram_start = ORIGIN(RAM);
__fae_ram_end   = __StackLimit;

ASSERT(__fae_ram_end >= __fae_ram_start, "Invalid RAM window for FAE")
