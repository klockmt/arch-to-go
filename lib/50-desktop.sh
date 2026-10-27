#!/usr/bin/env bash
# Hyprland, kitty, fish, nautilus, tuned e rede (iwd + systemd-networkd). Carregado pelo install.sh.

install_desktop() {
    local home=/mnt/home/$ARCH_USER

    run arch-chroot /mnt pacman -S --noconfirm --needed hyprland kitty fish nautilus tuned tuned-ppd iwd ly ttf-dejavu
    run arch-chroot /mnt usermod -s /usr/bin/fish "$ARCH_USER"

    run install -Dm644 "$ROOT_DIR/config/hypr/hyprland.conf" "$home/.config/hypr/hyprland.conf"
    run arch-chroot /mnt chown -R "$ARCH_USER:$ARCH_USER" "/home/$ARCH_USER/.config"

    # ly sobe a sessão Hyprland (hyprland.desktop) no tty1; getty@tty1 não pode disputar o tty
    run arch-chroot /mnt systemctl disable getty@tty1.service
    run arch-chroot /mnt systemctl enable "${LY_UNIT:-ly@tty1.service}"

    # iwd só cuida do Wi-Fi; o IP vem do systemd-networkd
    run install -Dm644 "$ROOT_DIR/config/iwd/main.conf" /mnt/etc/iwd/main.conf
    run install -Dm644 "$ROOT_DIR/config/networkd/20-dhcp.network" /mnt/etc/systemd/network/20-dhcp.network
    run arch-chroot /mnt ln -sf /run/systemd/resolve/stub-resolv.conf /etc/resolv.conf
    run arch-chroot /mnt systemctl enable iwd systemd-networkd systemd-resolved tuned tuned-ppd
}
