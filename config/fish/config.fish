# Sobe o Hyprland no tty1, sem display manager
if status is-login; and test -z "$WAYLAND_DISPLAY"; and test "$XDG_VTNR" = 1
    exec Hyprland
end
