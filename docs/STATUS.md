# Status

## Works (covered by tests that need no hardware)
- Config parsing and validation, safety checks (with `lsblk`/`findmnt` faked), plan building
  (disk, base system, Limine, profile/network/extras), runner (dry run with the password as `***`,
  mock, real), mount guard, Ctrl-C handling, CLI refusals.

## Run end to end
- **Real install** of the Hyprland profile (btrfs, `linux`, audio, iwd + networkd) onto a 111.8G
  USB SSD (`/dev/sda`), twice. The first run failed on two bugs (resolv.conf link inside
  arch-chroot, host mirrorlist copied into the target); both are fixed with regression tests
  (see `PITFALLS.md`). The second run finished with exit 0 and `/mnt` unmounted.
- **Disk contents checked** read-only: ESP with `EFI/BOOT/BOOTX64.EFI`, `limine.conf`, kernel,
  initramfs and microcode; hostname, fstab by UUID with the subvolumes, admin user with fish,
  sudoers mode 440, resolv.conf symlink, official mirrorlist, `ly@tty1` and networkd enabled,
  Hyprland config with the keymap, and the root PARTUUID in `limine.conf` equal to the disk's.
- **Boot test in QEMU** (OVMF, USB mass storage, snapshot mode so nothing is written): UEFI ->
  Limine -> Arch -> `ly` login screen with the hostname, the Hyprland session and the admin user.

## Not run yet
- Boot on real hardware, and on more than one machine (the portability claim).
- Logging in and starting the Hyprland session; Wi-Fi/DHCP on the installed system.
- Profiles `gnome`/`kde`, `network-manager`, `ext4`/`xfs`, multiple kernels, `zram`, `aur.paru`
  have plan tests only, no real install.
- `scripts/test-loop` has not run: the `loop` module is missing for the running kernel on the
  development host.

## Known gaps
- Deferred from the final review: `/mnt` is not checked empty before mounting; only Ctrl-C is
  handled (not SIGHUP/SIGTERM); `esp_size` has no lower bound; any btrfs subvolume name becomes a
  mountpoint (`@usr`, `@etc`, `@boot` would not boot); an unknown timezone or keymap fails late or
  falls back silently; the cached paru package is never aged out; the disk serial is not compared
  between the confirmation and the wipe; ZFS pools are not detected as host filesystems.
- `system.mirror_region` is validated but not applied.
- During pacstrap, pacman's own mkinitcpio hook prints `call to execv failed`; harmless here
  because the installer writes presets and runs `mkinitcpio -P` itself.
- No LUKS, no legacy BIOS, no TUI (out of scope for v1).
