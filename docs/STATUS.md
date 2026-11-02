# Status

## Works (covered by tests that need no hardware)
- Config parsing and validation (`cargo test -p arch-to-go-core --test config`).
- Safety checks, with `lsblk`/`findmnt` faked (`--test safety`).
- Plan building: disk, base system, Limine boot, profile/network/extras.
- Runner: dry run (password shown as `***`), mock, real; mount guard; Ctrl-C handling.
- CLI: `install --dry-run`, `check-config`, refusals (non-USB, host disk, fake system on a real run).

## Not run end to end yet
- `scripts/test-loop` (install onto a loop device) exists but has **not run**: on the
  development host the `loop` module is missing for the running kernel
  (`CONFIG_BLK_DEV_LOOP=m`, no `/lib/modules/<running kernel>`), so `losetup` fails
  with ENXIO. Reboot into a kernel with its modules, or reinstall them, then run
  `scripts/arch-sudo scripts/test-loop`.
- No real install on a USB disk has been done with the Rust installer.
- Nothing has been booted.

## Known gaps
- `system.mirror_region` is validated but not applied yet.
- `ly@tty1.service` (Hyprland profile) comes from the old bash script and is unverified.
