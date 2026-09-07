# mps2-an385

Startup FAE firmware pour `mps2-an385`.

## Build du startup ELF

```bash
arm-none-eabi-gcc -c -mcpu=cortex-m3 -mthumb rt0/bootable/mps2-an385/boot_rt0.s -o /tmp/boot_rt0_an385.o
arm-none-eabi-gcc -nostdlib -nostartfiles -Wl,--build-id=none \
  -T rt0/bootable/mps2-an385/link.ld \
  -mcpu=cortex-m3 -mthumb \
  /tmp/boot_rt0_an385.o -o /tmp/boot_rt0_an385.elf
```

## Build d'un FAE firmware

```bash
cargo run --bin build_fae -- \
  --startup-code /tmp/boot_rt0_an385.elf \
  --no-warn-entry-mode-mismatch \
  sdk/minimal-c/build/minimal.elf
```

## QEMU

```bash
qemu-system-arm -machine mps2-an385 -nographic \
  -semihosting-config enable=on,target=native \
  -kernel sdk/minimal-c/build/minimal.fae
```
