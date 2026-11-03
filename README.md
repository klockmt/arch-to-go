# arch-to-go

Installs a portable Arch Linux (UEFI x86_64) onto removable storage, in the spirit of
Windows To Go. It borrows archinstall's idea: a declarative config with the usual options,
or interactive prompts. A TUI will follow on the same core.

The drive boots on any 64-bit UEFI PC: Limine goes on the removable path
(`EFI/BOOT/BOOTX64.EFI`, no NVRAM write on the machine you install from) and the initramfs
has no `autodetect`.

> Status: see [docs/STATUS.md](docs/STATUS.md) for what has and has not been run.

## Build and run

```sh
cargo build --release
arch-to-go check-config my.toml
arch-to-go install /dev/sdX --config my.toml --dry-run   # prints the plan, runs nothing
sudo arch-to-go install /dev/sdX --config my.toml        # erases /dev/sdX
```

Without `--config` it asks the questions and builds the same config. `DRY_RUN=1` works like
`--dry-run`. The password is asked twice (hidden), or read from `ARCH_ADMIN_PASSWORD`; it never
goes in the config file (unknown keys are rejected) and is never printed. The admin account gets
`wheel`/sudo and the same password is set for root.

Host tools needed: `sgdisk`, `wipefs`, `partprobe`, `mkfs.fat`, `mkfs.btrfs` (or ext4/xfs
tools), `pacstrap`, `genfstab`, `arch-chroot`. For `aur.paru`: `git`, `makepkg`, and `sudo`
from a normal user (makepkg refuses root).

## TUI

```sh
arch-to-go tui [--config my.toml]
```

An archinstall-style menu: Disk, Boot, System, User, Profile, Network, Extras, then Save, Load,
Review and Install. Sections that need attention are flagged with `!`. The TUI edits the same
config the CLI uses, so the rules are identical; it needs a terminal of at least 80x24.

- **Disk** lists the whole USB disks it finds. **User** takes the admin name and the password
  twice (masked; the password is never written to the config or the log).
- **Review** shows the exact plan (`install --dry-run`) in a scrollable pane.
- **Install** asks you to type the device path, then runs the CLI and shows the steps and its
  output live. Ctrl-C aborts and waits for the CLI to clean up. Without root it tells you to save
  the config and gives the `sudo arch-to-go install ... --config ...` command.

The TUI never touches a disk itself: it runs `arch-to-go install` as a subprocess, so every
safety rule above stays in one place.

## Safety

Before anything is written the installer refuses a target that is not a whole USB disk, backs
a mounted filesystem of the running system, or has mounted partitions. You must then type the
exact device path. If it cannot inspect the system it refuses. `/mnt` is unmounted on exit and
on Ctrl-C.

## Config

```toml
[users]
admin = "me"            # required

[system]                # defaults shown
hostname = "archtogo"
locale = "pt_BR.UTF-8"
keymap = "br-abnt2"
timezone = "America/Sao_Paulo"

[disk]
fs = "btrfs"            # btrfs | ext4 | xfs
esp_size = "1G"
# subvolumes = ["@", "@home", "@log", "@pkg"]   # btrfs only
nodatacow = false       # turns off CoW and checksums; a USB error can corrupt data silently
zram = false

[boot]
kernels = ["linux"]     # linux | linux-lts | linux-zen

[profile]
name = "minimal"        # minimal | hyprland | gnome | kde
audio = false           # pipewire

[network]
backend = "iwd-networkd"  # iwd-networkd | network-manager

[aur]
paru = false

[portable]
firmware = true         # linux-firmware, microcode, sof-firmware
gpu_drivers = true      # mesa, vulkan-radeon, vulkan-intel, intel-media-driver

packages = ["vim"]      # extra packages (top level, before any [table])
services = ["sshd.service"]
```

`system.mirror_region` is not an option yet. The installed system uses the official Arch
mirrors from `crates/core/assets/pacman.conf`.

## Development

```sh
scripts/check-repo          # fmt, clippy, tests, shellcheck, private-data scan
scripts/arch-sudo scripts/test-loop   # end to end on a loop device (needs the loop module)
# ARCH_TO_GO_ALLOW_LOOP=1 makes the CLI and the TUI disk picker accept loop devices (tests only)
```

Local values (sudo password, test user and device) live in `.env`; copy `.env.example`.
Design: `docs/superpowers/specs/`, plan: `docs/superpowers/plans/`.
