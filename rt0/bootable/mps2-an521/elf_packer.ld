ENTRY(_start)

MEMORY
{
  NS_FLASH (rx)  : ORIGIN = 0x00200000, LENGTH = 512K
  NS_RAM   (rwx) : ORIGIN = 0x28200000, LENGTH = 32K
}

SECTIONS
{
  . = ORIGIN(NS_RAM);

  .text._start : ALIGN(4)
  {
    KEEP(*(.text._start))
    . = ALIGN(4);
  } > NS_RAM

  . = ORIGIN(NS_FLASH);

  .fae_blob : ALIGN(4)
  {
    KEEP(*(.fae_blob))
    . = ALIGN(4);
  } > NS_FLASH

  /DISCARD/ :
  {
    *(.ARM.exidx*)
    *(.eh_frame*)
    *(.comment)
    *(.note*)
    *(.ARM.attributes)
  }
}
