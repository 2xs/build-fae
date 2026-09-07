    .syntax unified
    .arm
    /* build_fae rebuilds the embedded startup ELFs from source while the host
     * tool is compiled. Rebuild build_fae after editing this startup to pick
     * the changes up in the default embedded flow.
     */

    .section ._start, "ax", %progbits
    .global _start
    .type _start, %function
_start:
    .include "rt0/arm-arm/rt0-arm.s"

    .global _end
_end:
