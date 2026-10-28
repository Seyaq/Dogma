# Dogma

Micro-noyau x86_64 en Rust (`no_std`, zéro dépendance) visant la performance maximale,
un TCB minimal et la sécurité. Objectif de référence : seL4, comparé sur des métriques
mesurées (cycles par aller-retour IPC, taille du TCB), pas sur des affirmations.

## État
- [x] Boot UEFI via Limine, console série COM1
- [x] GDT/TSS (pile IST pour #DF), IDT et exceptions CPU
- [x] Allocateur de frames physiques (bitmap)
- [x] Pagination propre (PML4 noyau, W^X, HHDM sans alias du code)
- [ ] Tas noyau
- [ ] Timer (LAPIC/TSC), ordonnanceur, threads
- [ ] Capabilities, IPC (fastpath), syscalls (SYSCALL/SYSRET)
- [ ] Premier processus utilisateur, mesures (`examples/benchmark.csv`)

## Build / test (nécessite Rust stable, QEMU, OVMF, Limine binaire)
```
git clone --branch=v9.x-binary --depth=1 https://github.com/limine-bootloader/limine
make run     # démarre sous QEMU (sortie sur le terminal)
make smoke   # vérifie que le noyau affiche « [dogma] boot ok »
```
