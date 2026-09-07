# olimex-stm32-h405

Startup FAE bootable pour `olimex-stm32-h405`.

## Build du startup ELF

```bash
arm-none-eabi-gcc -c -mcpu=cortex-m4 -mthumb rt0/bootable/olimex-stm32-h405/boot_rt0.s -o /tmp/boot_rt0_olimex_stm32_h405.o
arm-none-eabi-gcc -nostdlib -nostartfiles -Wl,--build-id=none \
  -T rt0/bootable/olimex-stm32-h405/link.ld \
  -mcpu=cortex-m4 -mthumb \
  /tmp/boot_rt0_olimex_stm32_h405.o -o /tmp/boot_rt0_olimex_stm32_h405.elf
```

## Build d'un FAE bootable

```bash
cargo run --bin build_fae -- \
  --firmware olimex-stm32-h405 \
  sdk/minimal-c/build/minimal.elf
```

## QEMU

```bash
qemu-system-arm -machine olimex-stm32-h405 -nographic \
  -semihosting-config enable=on,target=native \
  -kernel sdk/minimal-c/build/minimal.fae
```
