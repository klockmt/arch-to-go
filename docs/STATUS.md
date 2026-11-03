# Status

## Works (covered by tests that need no hardware)
- Config parsing and validation, safety checks (with `lsblk`/`findmnt` faked), plan building
  (disk, base system, Limine, profile/network/extras), runner (dry run with the password as `***`,
  mock, real), mount guard, Ctrl-C handling, CLI refusals.
- TUI (`crates/tui`): app state machine, section editors, review/confirm/run/done flow, child
  driver, drawing on a `TestBackend`, runtime effects, `arch-to-go tui` subcommand.

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

- **Loop-device end-to-end test** (`scripts/arch-sudo scripts/test-loop`): passes 10/10 checks with a
  clean unmount, no leftover loop device or gpg-agent (after fixing the agent that forced a lazy unmount).
- **Boot on real hardware:** the installed external SSD boots and works (confirmed by the user).
- **TUI driven in `tmux`** (the real binary, not mocks):
  - Unprivileged: the menu, the Disk/User editors (masked password, "passwords differ" message),
    the Review preview (the real CLI's dry run for the real disk), the "root needed" message
    with the `sudo` command, Save (valid TOML, no password) and the "cannot load" message.
  - As root, onto a loop device: pick the disk, set the user, Install showed "ready", the
    confirm screen refused a partial path, then the full install ran to "Install finished
    successfully"; afterwards `/mnt` unmounted, no gpg-agent, no temp config directory, no loop
    device left.
  - Ctrl-C in the middle of pacstrap: "aborting, waiting for cleanup", then "Install failed
    (exit 1)"; this exposed a lingering gpg-agent that is now stopped by the mount guard, and a
    second run left nothing behind (see `PITFALLS.md`).

## Not run yet
- Boot on more than one machine (the portability claim).
- The TUI on a real USB stick (only a loop device and the dry-run on the real disk were used),
  on a terminal other than tmux, and resizing below 80x24 while it runs.
- Logging in and starting the Hyprland session; Wi-Fi/DHCP on the installed system.
- Profiles `gnome`/`kde`, `network-manager`, `ext4`/`xfs`, multiple kernels, `zram`, `aur.paru`
  have plan tests only, no real install.

## Known gaps
- Deferred from the final review: `/mnt` is not checked empty before mounting; only Ctrl-C is
  handled (not SIGHUP/SIGTERM); `esp_size` has no lower bound; any btrfs subvolume name becomes a
  mountpoint (`@usr`, `@etc`, `@boot` would not boot); an unknown timezone or keymap fails late or
  falls back silently; the cached paru package is never aged out; the disk serial is not compared
  between the confirmation and the wipe; ZFS pools are not detected as host filesystems.
- `system.mirror_region` is validated but not applied.
- During pacstrap, pacman's own mkinitcpio hook prints `call to execv failed`; harmless here
  because the installer writes presets and runs `mkinitcpio -P` itself.
- TUI, deferred from the final review: no subvolumes editor (set them in the TOML; cycling the
  filesystem away from btrfs and back drops them); the disk list is read once, so a stick replugged
  under the same name is not noticed until the CLI's own recheck; the first `==>` step can be
  glued to the CLI's prompt and show as "starting"; the child's stdin stays open after the device
  path is sent; Abort could signal a reused process group in a tiny window after the child
  exited; the sudo hint can name a file whose Save failed; SIGHUP (terminal closed) is not
  handled; the dry run gets the password in its environment though it does not need it.
- TUI: Save accepts an invalid config (Load then rejects it); the log keeps every line (no cap);
  the disk list is read once at start; a child that dies from a signal reports 128+signal.
- No LUKS, no legacy BIOS (out of scope for v1).
