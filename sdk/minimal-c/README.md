# Minimal C Starter

Starter C minimal pour produire un payload FAE sans dependance `stdriot`.

Caracteristiques :

- `start(context_s *ctx)` comme point d'entree runtime
- semihosting direct via `bkpt 0xab`
- aucun `printf`, aucune libc, aucune couche historique XiPFS

## Build de l'ELF

Depuis la racine du depot :

```bash
make -C sdk/minimal-c
```

Le resultat est :

- `sdk/minimal-c/build/minimal.elf`

## Build du FAE

Avec le startup firmware `mps2-an521` :

```bash
arm-none-eabi-gcc -c -mcpu=cortex-m33 -mthumb rt0/bootable/mps2-an521/boot_rt0.s -o /tmp/boot_rt0.o
arm-none-eabi-gcc -nostdlib -nostartfiles -Wl,--build-id=none \
  -T rt0/bootable/mps2-an521/link.ld \
  -mcpu=cortex-m33 -mthumb \
  /tmp/boot_rt0.o -o /tmp/boot_rt0.elf

cargo run --bin build_fae -- \
  --startup-code /tmp/boot_rt0.elf \
  --no-warn-entry-mode-mismatch \
  sdk/minimal-c/build/minimal.elf
```

Le `ctx` est accepte par `start`, mais ce starter ne l'utilise pas encore : il affiche
simplement `Hello World!` via le semihosting.
