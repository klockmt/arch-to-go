#!/usr/bin/env bash
# Sistema base em /mnt: pacstrap, fstab, locale, hostname e usuário. Carregado pelo install.sh.

ARCH_HOSTNAME=${ARCH_HOSTNAME:-archtogo}
ARCH_LOCALE=${ARCH_LOCALE:-pt_BR.UTF-8}
ARCH_TZ=${ARCH_TZ:-America/Sao_Paulo}

# Pede o usuário (que também é root) e a senha. Roda antes de qualquer escrita no disco.
ask_account() {
    local pass pass2
    if [[ $DRY_RUN == 1 ]]; then
        ARCH_USER=${ARCH_USER:-usuario}
        ADMIN_PASS=dry-run
        return 0
    fi

    read -rp "Nome do usuário (também será root): " ARCH_USER
    [[ $ARCH_USER =~ ^[a-z_][a-z0-9_-]*$ ]] || die "nome inválido: use minúsculas, números, _ ou -"
    while :; do
        read -rsp "Senha: " pass; echo
        read -rsp "Repita a senha: " pass2; echo
        [[ -n $pass && $pass == "$pass2" ]] && break
        echo "senha vazia ou diferente, tente de novo" >&2
    done
    ADMIN_PASS=$pass
}

# set_password <conta>: a senha vai por stdin, então não aparece em argv
set_password() {
    if [[ $DRY_RUN == 1 ]]; then
        log "[dry-run] definir senha de $1"
        return 0
    fi
    printf '%s:%s\n' "$1" "$ADMIN_PASS" | arch-chroot /mnt chpasswd
}

install_base() {
    # -C: o pacman.conf do host tem repositórios do CachyOS, que não podem ir para o sistema instalado
    run pacstrap -C "$ROOT_DIR/config/pacman.conf" -K /mnt base linux linux-firmware btrfs-progs sudo
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
    set_password root
    set_password "$ARCH_USER"
    unset ADMIN_PASS
}
