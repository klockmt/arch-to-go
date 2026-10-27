#!/usr/bin/env bash
# Hyprland, kitty, fish, nautilus, tuned e rede (iwd + systemd-networkd). Carregado pelo install.sh.

install_desktop() {
    local home=/mnt/home/$ARCH_USER

    run arch-chroot /mnt pacman -S --noconfirm --needed hyprland kitty fish nautilus tuned tuned-ppd iwd ttf-dejavu
    run arch-chroot /mnt usermod -s /usr/bin/fish "$ARCH_USER"

    # Sem display manager: o fish sobe o Hyprland no tty1
    run install -Dm644 "$ROOT_DIR/config/hypr/hyprland.conf" "$home/.config/hypr/hyprland.conf"
    run install -Dm644 "$ROOT_DIR/config/fish/config.fish" "$home/.config/fish/config.fish"
    run arch-chroot /mnt chown -R "$ARCH_USER:$ARCH_USER" "/home/$ARCH_USER/.config"

    # iwd só cuida do Wi-Fi; o IP vem do systemd-networkd
    run install -Dm644 "$ROOT_DIR/config/iwd/main.conf" /mnt/etc/iwd/main.conf
    run install -Dm644 "$ROOT_DIR/config/networkd/20-dhcp.network" /mnt/etc/systemd/network/20-dhcp.network
    run arch-chroot /mnt ln -sf /run/systemd/resolve/stub-resolv.conf /etc/resolv.conf
    run arch-chroot /mnt systemctl enable iwd systemd-networkd systemd-resolved tuned tuned-ppd
}
