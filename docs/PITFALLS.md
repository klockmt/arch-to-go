# Pitfalls

What broke, why, and the test that keeps it from breaking again.

| What broke | Why | Guarded by |
|---|---|---|
| `ln -sf ... /etc/resolv.conf` inside `arch-chroot` fails: Device or resource busy | arch-chroot bind-mounts the host's resolv.conf there | `plan_profile::iwd_networkd_writes_both_configs_and_resolv_symlink` |
| Target used the host's mirrorlist (CachyOS mirror, 404s) | pacstrap copies the host's `/etc/pacman.d/mirrorlist` | `plan_base::target_mirrorlist_is_official_and_written_after_pacstrap` |
