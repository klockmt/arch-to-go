#!/usr/bin/env bash
# paru pré-compilado: compila no host como usuário comum e instala o pacote no alvo. Carregado pelo install.sh.

PKG_CACHE="$ROOT_DIR/pkgs"

build_paru() {
    local user=${SUDO_USER:-$USER} build="$ROOT_DIR/build/paru"
    [[ $user != root ]] || die "rode com sudo a partir de um usuário normal: o makepkg não roda como root"
    if ! command -v makepkg >/dev/null || ! command -v git >/dev/null; then
        die "faltam makepkg e git no host"
    fi

    # Já existe pacote compilado: não recompila
    if [[ $DRY_RUN == 0 ]] && compgen -G "$PKG_CACHE/paru-*.pkg.tar.zst" >/dev/null; then
        log "paru já compilado em $PKG_CACHE"
        return 0
    fi

    run runuser -u "$user" -- mkdir -p "$ROOT_DIR/build" "$PKG_CACHE"
    run rm -rf "$build"
    run runuser -u "$user" -- git clone --depth 1 https://aur.archlinux.org/paru.git "$build"
    run runuser -u "$user" -- bash -c "cd '$build' && makepkg --noconfirm"
    run runuser -u "$user" -- bash -c "mv '$build'/paru-*.pkg.tar.zst '$PKG_CACHE'/"
}

install_paru() {
    run cp "$PKG_CACHE"/paru-*.pkg.tar.zst /mnt/root/
    run arch-chroot /mnt bash -c 'pacman -U --noconfirm /root/paru-*.pkg.tar.zst'
}
