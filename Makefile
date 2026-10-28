.RECIPEPREFIX = >
KERNEL := target/x86_64-unknown-none/release/dogma
ESP    := target/esp
# Clone de Limine (branche binaire) contenant BOOTX64.EFI : git clone --branch=v9.x-binary --depth=1 https://github.com/limine-bootloader/limine
LIMINE ?= limine
# Le chemin d'OVMF dépend de la distribution (Fedora : /usr/share/edk2/ovmf/OVMF_CODE.fd)
OVMF   ?= /usr/share/OVMF/OVMF_CODE.fd
QEMU   ?= qemu-system-x86_64
QFLAGS := -machine q35 -m 512M -cpu max \
  -drive if=pflash,format=raw,readonly=on,file=$(OVMF) \
  -drive format=raw,file=fat:rw:$(ESP) \
  -serial stdio -display none -no-reboot

.PHONY: kernel esp run smoke clean
kernel:
> cargo build --release

esp: kernel
> mkdir -p $(ESP)/EFI/BOOT $(ESP)/boot
> cp $(LIMINE)/BOOTX64.EFI $(ESP)/EFI/BOOT/BOOTX64.EFI
> cp limine.conf $(ESP)/limine.conf
> cp $(KERNEL) $(ESP)/boot/dogma

run: esp
> $(QEMU) $(QFLAGS)

# Test de fumée : le noyau doit afficher « boot ok » sur le port série.
smoke: esp
> timeout 20 $(QEMU) $(QFLAGS) | tee target/serial.log || true
> grep -q "\[dogma\] boot ok" target/serial.log

clean:
> cargo clean
