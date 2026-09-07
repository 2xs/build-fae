# pico2-rp2350

Startup FAE firmware pour Raspberry Pi Pico 2 / RP2350.

Ce premier profil vise surtout le bring-up materiel:

- Cortex-M33, un seul coeur pour l'instant
- boot direct depuis la flash XIP
- SRAM vue comme une plage unique commencant a `0x20000000`
- pas de separation TrustZone pour ce premier essai

## Build du startup ELF

```bash
arm-none-eabi-gcc -c -mcpu=cortex-m33 -mthumb \
  rt0/bootable/pico2-rp2350/boot_rt0.s -o /tmp/boot_rt0_pico2.o

arm-none-eabi-gcc -nostdlib -nostartfiles -Wl,--build-id=none \
  -T rt0/bootable/pico2-rp2350/link.ld \
  -mcpu=cortex-m33 -mthumb \
  /tmp/boot_rt0_pico2.o -o /tmp/boot_rt0_pico2.elf
```

## Build d'un FAE firmware

```bash
cargo run --bin build_fae -- \
  --startup-code /tmp/boot_rt0_pico2.elf \
  --no-warn-entry-mode-mismatch \
  sdk/minimal-c/build/minimal.elf
```

## Validation

QEMU n'est pas aujourd'hui la voie principale pour cette cible.

Le chemin vise ici plutot:

- build du `.fae`
- flash sur la Pico 2
- debug via JTAG/SWD et GDB

Le wrapper bootable commun masque les interruptions dans `Reset_Handler`.
Le `start()` du payload doit les reactiver explicitement si le firmware en a
besoin.

## Notes

Les adresses utilisees ici sont volontairement simples pour un premier port:

- flash XIP: `0x10000000`
- SRAM: `0x20000000`

Si le bring-up montre qu'il faut modeliser plus finement les banks SRAM ou le
demarrage secure/non-secure du RP2350, ce profil devra etre ajuste.
