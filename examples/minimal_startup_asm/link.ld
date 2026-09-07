ENTRY(_start)

SECTIONS
{
    . = 0x0;

    ._start : ALIGN(4)
    {
        KEEP(*("._start"))
        . = ALIGN(4);
    }

    /DISCARD/ :
    {
        *(.comment)
        *(.ARM.attributes)
    }
}
