#!/usr/bin/env sh
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
OUT_DIR="$ROOT/build/embedded-startups"
TMP_DIR="${TMPDIR:-/tmp}/xiprfs-embedded-startups.$$"

cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

mkdir -p "$OUT_DIR" "$TMP_DIR"

cd "$ROOT"

echo "Building embedded default startup ELFs..."

arm-none-eabi-gcc -c -mcpu=cortex-m4 -mthumb -I"$ROOT" \
    "$ROOT/rt0/arm-thumb/core.s" \
    -o "$TMP_DIR/rt0-thumb.o"
arm-none-eabi-ld -T "$ROOT/rt0/arm-thumb/link.ld" \
    "$TMP_DIR/rt0-thumb.o" \
    -o "$OUT_DIR/rt0-thumb.elf"

arm-none-eabi-gcc -c -march=armv7-a -I"$ROOT" \
    "$ROOT/rt0/arm-arm/core.s" \
    -o "$TMP_DIR/rt0-arm.o"
arm-none-eabi-ld -T "$ROOT/rt0/arm-arm/link.ld" \
    "$TMP_DIR/rt0-arm.o" \
    -o "$OUT_DIR/rt0-arm.elf"

echo "Building embedded bootable Cortex-M startup ELFs..."

arm-none-eabi-gcc -c -mcpu=cortex-m3 -mthumb -I"$ROOT" \
    "$ROOT/rt0/bootable/mps2-an385/boot_rt0.s" \
    -o "$TMP_DIR/mps2-an385.o"
arm-none-eabi-ld -T "$ROOT/rt0/bootable/mps2-an385/link.ld" \
    "$TMP_DIR/mps2-an385.o" \
    -o "$OUT_DIR/boot-mps2-an385.elf"

arm-none-eabi-gcc -c -mcpu=cortex-m4 -mthumb -I"$ROOT" \
    "$ROOT/rt0/bootable/mps2-an386/boot_rt0.s" \
    -o "$TMP_DIR/mps2-an386.o"
arm-none-eabi-ld -T "$ROOT/rt0/bootable/mps2-an386/link.ld" \
    "$TMP_DIR/mps2-an386.o" \
    -o "$OUT_DIR/boot-mps2-an386.elf"

arm-none-eabi-gcc -c -mcpu=cortex-m4 -mthumb -I"$ROOT" \
    "$ROOT/rt0/bootable/olimex-stm32-h405/boot_rt0.s" \
    -o "$TMP_DIR/olimex-stm32-h405.o"
arm-none-eabi-ld -T "$ROOT/rt0/bootable/olimex-stm32-h405/link.ld" \
    "$TMP_DIR/olimex-stm32-h405.o" \
    -o "$OUT_DIR/boot-olimex-stm32-h405.elf"

arm-none-eabi-gcc -c -mcpu=cortex-m4 -mthumb -I"$ROOT" \
    "$ROOT/rt0/bootable/b-l475e-iot01a/boot_rt0.s" \
    -o "$TMP_DIR/b-l475e-iot01a.o"
arm-none-eabi-ld -T "$ROOT/rt0/bootable/b-l475e-iot01a/link.ld" \
    "$TMP_DIR/b-l475e-iot01a.o" \
    -o "$OUT_DIR/boot-b-l475e-iot01a.elf"

echo "Generated startup ELFs in $OUT_DIR"
