    .syntax unified
    .thumb
    /* build_fae rebuilds the embedded startup ELFs from source while the host
     * tool is compiled. Rebuild build_fae after editing this startup to pick
     * the changes up in the default embedded flow.
     */

    .section ._start, "ax", %progbits
    .global _start
    .type _start, %function
    .thumb_func
_start:
    .include "rt0/arm-thumb/rt0-thumb.s"

    .global _end
_end:
