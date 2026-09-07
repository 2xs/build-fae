# mps2-an521

Startup FAE firmware pour `mps2-an521`.

## Build du startup ELF

```bash
arm-none-eabi-gcc -c -mcpu=cortex-m33 -mthumb rt0/bootable/mps2-an521/boot_rt0.s -o /tmp/boot_rt0_an521.o
arm-none-eabi-gcc -nostdlib -nostartfiles -Wl,--build-id=none \
  -T rt0/bootable/mps2-an521/link.ld \
  -mcpu=cortex-m33 -mthumb \
  /tmp/boot_rt0_an521.o -o /tmp/boot_rt0_an521.elf
```

## Build d'un FAE firmware

```bash
cargo run --bin build_fae -- \
  --startup-code /tmp/boot_rt0_an521.elf \
  --no-warn-entry-mode-mismatch \
  sdk/minimal-c/build/minimal.elf
```

## QEMU

Le boot direct du `.fae` ne semble pas fiable sur `mps2-an521` avec QEMU
`10.2.1`.

Le workaround est [elf_packer.s](elf_packer.s),
qui emballe le `.fae` dans un ELF de lancement pour QEMU tout en laissant le
contenu FAE inchange.

Build du packer :

```bash
arm-none-eabi-gcc -c -mcpu=cortex-m33 -mthumb -x assembler-with-cpp \
  -DFAE_PATH='"/absolute/path/to/app.fae"' \
  rt0/bootable/mps2-an521/elf_packer.s -o /tmp/elf_packer.o

arm-none-eabi-gcc -nostdlib -nostartfiles -Wl,--build-id=none \
  -T rt0/bootable/mps2-an521/elf_packer.ld \
  -mcpu=cortex-m33 -mthumb \
  /tmp/elf_packer.o -o /tmp/app-packer.elf
```

Execution QEMU :

```bash
qemu-system-arm -machine mps2-an521 -nographic \
  -semihosting-config enable=on,target=native \
  -device loader,file=/tmp/app-packer.elf,cpu-num=0
```
