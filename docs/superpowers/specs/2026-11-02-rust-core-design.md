# arch-to-go: Rust core design

## Goal

arch-to-go is "Windows To Go" for Arch Linux: it installs a rolling Arch
system onto removable storage that boots on any 64-bit UEFI PC. Its edge
is archinstall-style abstraction: a declarative config with archinstall's
options, plus an interactive prompt flow. A TUI follows once the core is
stable, on the same core.

## Decisions (agreed with the user)

- Language: Rust, Cargo workspace.
- Target hardware: UEFI x86_64 only, no legacy BIOS.
- Config: typed TOML file is the single source of truth. Without a file,
  a prompt flow fills the same struct. The TUI will edit the same struct.
- No LUKS in v1.
- Work happens on branch `beta`; merge to `main` only after the user
  validates.
- Test device is `$ARCH_TEST_DEVICE` from `.env`; root via
  `$ARCH_SUDO_PASSWORD`, test account from `$ARCH_USER_NAME`.

## Architecture

```
crates/core   config model, validation, plan, steps, safety, Runner trait
crates/cli    `arch-to-go` binary: args, prompts, wiring
crates/tui    (later) ratatui front end over core
```

Data flow: `Config (TOML)` -> `Plan` (ordered `Vec<Step>`) -> `Executor`.

- Building a plan is pure and deterministic: unit tests snapshot it.
- `--dry-run` / `DRY_RUN=1` prints the plan and runs nothing.
- Executor runs each step through a `Runner` trait. Tests use a mock
  runner; only the real runner touches the system.
- The installer shells out to host tools (`sgdisk`, `mkfs.*`, `pacstrap`,
  `genfstab`, `arch-chroot`, `limine`). It does not reimplement them and
  does not wrap archinstall.

## Config sections (v1)

| Section | Options |
|---|---|
| `disk` | target, filesystem (btrfs/ext4/xfs), btrfs subvolumes, ESP size, zram swap |
| `boot` | Limine on the removable path, kernel list (`linux`, `linux-lts`, `linux-zen`) |
| `system` | hostname, locale, keymap, timezone, mirror region |
| `users` | admin user with sudo; password from prompt or env, never in the file |
| `profile` | minimal, hyprland, gnome, kde, ...; audio |
| `network` | iwd + systemd-networkd, or NetworkManager |
| `packages`, `services` | extra lists |
| `aur` | optional paru |
| `portable` | generic initramfs (no autodetect), firmware and GPU driver sets |

Validation rejects unknown keys, impossible combinations and missing
required values before any step runs.

## Safety (ported from the bash installer, kept as invariants)

- Refuse anything that is not a whole USB disk.
- Refuse any disk backing a mounted host filesystem; fail closed if it
  cannot be listed.
- Refuse targets with mounted partitions.
- Require the user to type the exact device path to continue.
- Unmount `/mnt` on exit, with a lazy fallback.
- Secrets never appear in logs, plans, dry-run output or commits.

## Testing

- Unit: config parsing/validation, plan snapshots, safety checks with
  mocked `lsblk` / `findmnt` output.
- Integration: full run against a loop device.
- Real: runs on `$ARCH_TEST_DEVICE`, only after dry-run and loop tests pass.
- `scripts/check-repo` runs fmt, clippy, tests, shellcheck and a scan for
  personal data before each commit.
- Every bug gets a regression test, committed with or before the fix.

## Repo conventions (from hanoip-pmos-server)

`.env` / `.env.example`, `DRY_RUN=1` on anything mutating, `docs/STATUS.md`
(what works, honest about what ran end to end), `docs/PITFALLS.md` (what
broke and the test that guards it), `CLAUDE.md` and `AGENTS.md` carrying
the same rules, small commits (`type(scope): summary`), no personal data in
tracked files.

## Migration

Port the bash installer step by step into `core`, keeping behavior
(partition layout, subvolumes, Limine removable install and pacman hook,
networking, paru). Embed `config/*` templates in the binary. Delete the
bash code once the Rust version installs successfully on the test device.

## Out of scope for v1

LUKS, legacy BIOS, Apple hardware quirks, bootloaders other than Limine,
the TUI itself (design the core so it can plug in).
