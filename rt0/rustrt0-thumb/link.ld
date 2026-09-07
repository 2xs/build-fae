ENTRY(_start)

SECTIONS
{
  . = 0x0;

  ._start : ALIGN(4) {
    KEEP(*("._start"))
    *(.text .text.*)
    *(.rodata .rodata.*)
    *(.glue_7t) *(.glue_7)
    . = ALIGN(4);
  }

  __metadataOff = .;

  /DISCARD/ : {
    *(.ARM.exidx*)
    *(.eh_frame*)
    *(.comment)
    *(.ARM.attributes)
  }
}
