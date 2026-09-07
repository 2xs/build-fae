# b-l475e-iot01a

Startup FAE bootable pour `b-l475e-iot01a`.

## Build du startup ELF

```bash
arm-none-eabi-gcc -c -mcpu=cortex-m4 -mthumb rt0/bootable/b-l475e-iot01a/boot_rt0.s -o /tmp/boot_rt0_b_l475e_iot01a.o
arm-none-eabi-gcc -nostdlib -nostartfiles -Wl,--build-id=none \
  -T rt0/bootable/b-l475e-iot01a/link.ld \
  -mcpu=cortex-m4 -mthumb \
  /tmp/boot_rt0_b_l475e_iot01a.o -o /tmp/boot_rt0_b_l475e_iot01a.elf
```

## Build d'un FAE bootable

```bash
cargo run --bin build_fae -- \
  --firmware b-l475e-iot01a \
  sdk/minimal-c/build/minimal.elf
```

## QEMU

```bash
qemu-system-arm -machine b-l475e-iot01a -nographic \
  -semihosting-config enable=on,target=native \
  -kernel sdk/minimal-c/build/minimal.fae
```
