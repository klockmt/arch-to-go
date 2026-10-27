#!/usr/bin/env bash
# Sistema base em /mnt: pacstrap, fstab, locale, hostname e usuário. Carregado pelo install.sh.

ARCH_HOSTNAME=${ARCH_HOSTNAME:-archtogo}
ARCH_USER=${ARCH_USER:-klock}
ARCH_LOCALE=${ARCH_LOCALE:-pt_BR.UTF-8}
ARCH_TZ=${ARCH_TZ:-America/Sao_Paulo}

install_base() {
    run pacstrap -K /mnt base linux linux-firmware btrfs-progs sudo
    # fstab por UUID (genfstab -U). O redirecionamento fica dentro do bash -c para o dry-run não gravar
    run bash -c 'genfstab -U /mnt >> /mnt/etc/fstab'
}

configure_base() {
    # Sem autodetect: a imagem inicial serve para outras máquinas, não só esta
    run arch-chroot /mnt bash -c "
        set -e
        ln -sf /usr/share/zoneinfo/$ARCH_TZ /etc/localtime
        hwclock --systohc
        sed -i 's/^#\(en_US.UTF-8\|$ARCH_LOCALE\) UTF-8/\1 UTF-8/' /etc/locale.gen
        locale-gen
        echo LANG=$ARCH_LOCALE > /etc/locale.conf
        echo $ARCH_HOSTNAME > /etc/hostname
        sed -i 's/^HOOKS=.*/HOOKS=(base udev block keyboard keymap filesystems fsck)/; s/^MODULES=.*/MODULES=(btrfs)/' /etc/mkinitcpio.conf
        mkinitcpio -P
        useradd -m -G wheel $ARCH_USER
        echo '%wheel ALL=(ALL:ALL) ALL' > /etc/sudoers.d/10-wheel
        chmod 440 /etc/sudoers.d/10-wheel
    "
    run arch-chroot /mnt passwd
    run arch-chroot /mnt passwd "$ARCH_USER"
}
