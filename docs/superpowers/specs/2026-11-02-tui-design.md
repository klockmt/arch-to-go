# arch-to-go TUI design

## Goal

A terminal UI for the installer, in the archinstall style: a main menu of option sections, a disk
picker, a plan preview, a confirmation, and a live install view. It edits the same `Config` the CLI
and the prompts use, so validation lives in one place.

## Decisions (agreed with the user)

- Flow: archinstall-style menu (sections in any order, then Install).
- Privileges: the TUI starts as any user. Install needs root; without root it offers to save the
  config and shows the exact `sudo arch-to-go install ... --config ...` command. No escalation code.
- Architecture: the TUI is a front end that drives the CLI as a subprocess. It never touches a disk,
  so every safety rule stays in the CLI (USB-only, never a host disk, exact typed path, recheck
  before the wipe, unmount on exit).
- Stack: Rust, `ratatui` with its crossterm backend. Work on `beta`; merge to `main` only after the
  user validates.

## Architecture

```
crates/tui   library: app state machine, screens, drawing, child driver
crates/cli   gains `arch-to-go tui [--config FILE]` (one binary on a live ISO)
crates/core  Config becomes Serialize; Config::to_toml()
```

`crates/tui` depends on `arch-to-go-core` (only for `Config` and `Secret`), `ratatui`, `tempfile`.
It exposes `run(initial: Option<Config>) -> Result<(), String>`.

### App state machine (pure, no terminal)

`App { screen, config, device: Option<String>, password: Option<Secret>, errors, log, ... }`.
`App::handle_key(KeyEvent) -> Effect`, where `Effect` is one of: `None`, `Quit`,
`Review`, `Install`, `Abort`, `Save(path)`, `Load(path)`. The event loop is a thin shell that runs
effects and feeds results back as `App` events. Drawing is a pure function `draw(&App, &mut Frame)`.

### Screens

| Screen | Content |
|---|---|
| Main menu | Disk, Boot, System, User, Profile, Network, Extras, Save, Load, Review, Install, Quit. Each section shows its current value and a red hint when invalid. Install is enabled only when `Config::validate` passes, a disk is picked and a password is set. |
| Disk | Whole USB disks from `lsblk` (name, size, model), then fs (btrfs/ext4/xfs), ESP size, subvolumes, nodatacow, zram. |
| Boot | Kernel multi-select (linux, linux-lts, linux-zen). |
| System | Hostname, locale, keymap, timezone. |
| User | Admin name, masked password twice. The password is held as a `Secret` and never written anywhere. |
| Profile | minimal / hyprland / gnome / kde, and audio. |
| Network | iwd-networkd / network-manager. |
| Extras | packages, services, aur.paru, portable.firmware, portable.gpu_drivers. |
| Review | The `--dry-run` plan in a scrollable pane. |
| Confirm | The user types the exact device path. |
| Run | Current step, scrollable log, Ctrl-C to abort. |
| Done | Exit status and the last log lines; the CLI's "wait and run sync" warning is shown when present. |

Every edit re-validates through `Config::validate`; the TUI holds no copy of the rules. Minimum
terminal size is 80x24; below it the screen says so instead of drawing.

## Child driver

- Binary: `std::env::current_exe()` (tests pass a stub path instead).
- Config: written to a private temp directory (mode 0700) as `config.toml`, deleted afterwards. It
  holds no secret (the config type has no password field).
- Review: `<exe> install DEV --config F --dry-run`, stdout collected, no root needed.
- Install: `<exe> install DEV --config F` with `ARCH_ADMIN_PASSWORD` in the child's environment and
  stdin/stdout/stderr piped. After the user confirms the path in the TUI, the TUI writes it plus a
  newline to the child's stdin, which is what the CLI's own prompt reads; the CLI still verifies it.
- Output: two reader threads send lines over an `mpsc` channel. A line starting `==> ` is a step and
  updates progress; every line goes to the log. A trailing partial line (the CLI's prompt) is
  flushed at EOF.
- Abort: Ctrl-C arrives as a key event (raw mode), the TUI runs `kill -INT <pid>`, shows
  "aborting, waiting for cleanup", and waits for the child to exit before leaving the Run screen.
- Root check: effective uid read as the CLI does it; not root disables the Install action.

## Safety and errors

- The TUI never mounts, partitions or writes a disk. No safety logic is duplicated.
- The terminal is restored on normal exit, on error and on panic (hook).
- A failed child shows its exit status and the last log lines; the temp config is removed.
- Secrets: the password never reaches a file, the log or an argv. It is in the child's environment
  only, like using the CLI with `ARCH_ADMIN_PASSWORD`.

## Testing

- App state machine: key sequences in, state and effects out (menu navigation, edits validate,
  Install disabled until valid, password mismatch, confirm requires the exact path, abort).
- Drawing: ratatui `TestBackend` buffers contain the expected text per screen, and the too-small
  message under 80x24.
- Parsers: `lsblk -P` output to the disk list (only whole USB disks), and `==> ` progress lines.
- Config round trip: `Config::to_toml` then `from_toml` gives an equal config, and the output has no
  password key.
- Child driver: against a stub script that prints `==> step` lines, reads a confirmation line from
  stdin, and exits 0 or 1; covers success, failure, abort and the partial prompt line.
- Manual end-to-end after the unit tests: the TUI inside a terminal multiplexer, driving a real
  install to a loop device, with the screen captured.

## Docs

README gets a TUI section; `docs/STATUS.md` says exactly what was run; `docs/PITFALLS.md` gets a row
for every bug found.

## Out of scope for v1

Package search, mouse support, themes, localization, resizing beyond the 80x24 minimum, running the
install in-process, escalating to root from inside the TUI.
