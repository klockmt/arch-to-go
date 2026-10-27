#!/usr/bin/env bash
# Limine como bootloader removível (EFI/BOOT/BOOTX64.EFI). Não grava na NVRAM do host. Carregado pelo install.sh.

install_limine() {
    local disk=$1 uuid
    if [[ $DRY_RUN == 1 ]]; then
        uuid=DRY-RUN-UUID
    else
        uuid=$(blkid -s UUID -o value "$(part "$disk" 2)")
        [[ -n $uuid ]] || die "não consegui ler o UUID de $(part "$disk" 2)"
    fi

    run arch-chroot /mnt pacman -S --noconfirm --needed limine
    run mkdir -p /mnt/boot/EFI/BOOT /mnt/etc/pacman.d/hooks
    run cp /mnt/usr/share/limine/BOOTX64.EFI /mnt/boot/EFI/BOOT/BOOTX64.EFI
    run bash -c "sed 's/@ROOT_UUID@/$uuid/' '$ROOT_DIR/config/limine.conf.in' > /mnt/boot/limine.conf"
    # Recopia o EFI quando o pacote limine atualiza, senão o boot continua com a versão antiga
    run cp "$ROOT_DIR/config/limine.hook" /mnt/etc/pacman.d/hooks/limine.hook
}
