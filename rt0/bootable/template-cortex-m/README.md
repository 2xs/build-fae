# template-cortex-m

Template a copier pour une nouvelle cible firmware Cortex-M.

Points a ajuster :

- `boot_rt0.s` : `board_name` et eventuellement `.arch`
- `link.ld` : adresses et tailles de `FLASH` / `RAM`
- la commande QEMU de la cible
