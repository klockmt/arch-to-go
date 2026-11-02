# arch-to-go

Installer that puts a portable Arch Linux (UEFI x86_64) on removable storage, like Windows To Go, with archinstall-style options driven by a TOML config or prompts. A TUI comes later on the same core. Reply to the user in Portuguese; commits, code and docs are in English.

Read `docs/STATUS.md` (what works) and `docs/PITFALLS.md` (what broke) first, then the spec and plan under `docs/superpowers/`.

## Rules that always apply
- Run `scripts/check-repo` before every commit and push.
- Local paths and personal data live in `.env` (copy of `.env.example`). Scripts source `scripts/env.sh`; docs use `$ARCH_*` variables. Never write a password, user name, home path, serial or MAC in any tracked file, fixture, log or commit message.
- Everything that changes the system honors `--dry-run` / `DRY_RUN=1`. Keep `docs/STATUS.md` honest about what ran end to end.
- The test device is `$ARCH_TEST_DEVICE`, the test account is `$ARCH_USER_NAME`. Root commands go through `scripts/arch-sudo` (password from `.env`, over stdin); never print or log it. Check the device with `lsblk` and tell the user what is on it before wiping it.
- Safety invariants never weaken: whole USB disk only, never a host disk, never with mounted partitions, the user types the exact path, `/mnt` is unmounted on exit.
- Passwords reach `chpasswd` over stdin only. Config values never go through `sh -c`; use argv vectors.
- Report test results as they are, failures included.

## Tests for every bug
- Add the regression test before or with the fix; watch it fail on the old behavior, then pass. Add a row to `docs/PITFALLS.md`.

## Commits and branches
Small, one per specific change, `type(scope): summary` with a body saying why. Never mention AI, no trailers. Author is the git config identity.

Work on `beta`. Merge to `main` only after the user validated; never push or merge without asking.
