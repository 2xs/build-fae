ENTRY(_start)

__STACK_SIZE_CPU0 = 0x00004000;

MEMORY
{
  NS_FLASH (rx)  : ORIGIN = 0x00200000, LENGTH = 512K
  NS_RAM   (rwx) : ORIGIN = 0x28200000, LENGTH = 32K
}

SECTIONS
{
  . = ORIGIN(NS_FLASH);

  ._start : ALIGN(4)
  {
    KEEP(*("._start"))
    . = ALIGN(4);
  } > NS_FLASH

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

__StackTop      = ORIGIN(NS_RAM) + LENGTH(NS_RAM);
__StackLimit    = __StackTop - __STACK_SIZE_CPU0;
__fae_ram_start = ORIGIN(NS_RAM);
__fae_ram_end   = __StackLimit;

ASSERT(__fae_ram_end >= __fae_ram_start, "Invalid RAM window for FAE")
