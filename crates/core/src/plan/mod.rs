//! Builds the ordered install steps from a validated `Config`. Pure: no I/O.

pub mod base;
pub mod boot;
pub mod disk;
pub mod profile;

use crate::config::{Config, ConfigError};
use crate::step::{Cmd, Secret, Step};

/// Run-time facts the config cannot hold.
pub struct Ctx {
    /// Target disk, e.g. `/dev/sda`.
    pub disk: String,
    /// GUID given to the root partition at partition time, so the boot entry
    /// can name it without reading it back from the disk.
    pub root_partuuid: String,
    pub admin_password: Secret,
    /// Host path of a prebuilt paru package, needed when `aur.paru` is set.
    pub paru_pkg: Option<String>,
}

impl Ctx {
    /// Validates what reaches `chpasswd` and the partition table.
    pub fn new(disk: &str, root_partuuid: &str, password: &str) -> Result<Ctx, ConfigError> {
        let bad = |m: &str| Err(ConfigError::Invalid(m.into()));
        if !disk.starts_with("/dev/") || disk.contains(char::is_whitespace) {
            return bad("disk must be a /dev/ path");
        }
        if !is_uuid(root_partuuid) {
            return bad("root_partuuid must be a UUID (8-4-4-4-12 hex)");
        }
        // chpasswd reads one `user:password` line; a newline would add a second entry
        if password.is_empty() || password.contains(['\n', '\r', '\0']) {
            return bad("password must be non-empty and have no newline");
        }
        Ok(Ctx {
            disk: disk.into(),
            root_partuuid: root_partuuid.into(),
            admin_password: Secret::new(password),
            paru_pkg: None,
        })
    }

    pub fn with_paru_pkg(mut self, path: impl Into<String>) -> Ctx {
        self.paru_pkg = Some(path.into());
        self
    }
}

fn is_uuid(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.iter().map(|g| g.len()).eq([8, 4, 4, 4, 12])
        && groups
            .iter()
            .all(|g| g.chars().all(|c| c.is_ascii_hexdigit()))
}

pub fn build(cfg: &Config, ctx: &Ctx) -> Result<Vec<Step>, ConfigError> {
    if cfg.aur.paru && ctx.paru_pkg.is_none() {
        return Err(ConfigError::Invalid(
            "aur.paru needs a prebuilt paru package (the CLI builds it on the host)".into(),
        ));
    }
    let mut steps = disk::disk_steps(cfg, ctx);
    steps.extend(base::base_steps(cfg, ctx));
    steps.extend(boot::boot_steps(cfg, ctx));
    steps.extend(profile::profile_steps(cfg, ctx));
    // pacstrap and the chroot pacman calls start a gpg-agent on the target's keyring; left
    // running it holds /mnt, so the unmount could only be lazy
    steps.push(run(
        "stop gpg-agent",
        &[
            "gpgconf",
            "--homedir",
            "/mnt/etc/pacman.d/gnupg",
            "--kill",
            "all",
        ],
    ));
    // flush before the caller reports success: the stick may be unplugged right after
    steps.push(run("flush writes to the disk", &["sync"]));
    Ok(steps)
}

/// A command step.
pub(crate) fn run(desc: &str, argv: &[&str]) -> Step {
    Step::Run {
        desc: desc.into(),
        cmd: Cmd::new(argv),
    }
}

/// A file write step.
pub(crate) fn write(desc: &str, path: &str, contents: &str, mode: u32) -> Step {
    Step::WriteFile {
        desc: desc.into(),
        path: path.into(),
        contents: contents.into(),
        mode,
    }
}
