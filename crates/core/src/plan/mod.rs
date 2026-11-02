//! Builds the ordered install steps from a validated `Config`. Pure: no I/O.

pub mod base;
pub mod disk;

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
        })
    }
}

fn is_uuid(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.iter().map(|g| g.len()).eq([8, 4, 4, 4, 12])
        && groups
            .iter()
            .all(|g| g.chars().all(|c| c.is_ascii_hexdigit()))
}

pub fn build(cfg: &Config, ctx: &Ctx) -> Vec<Step> {
    let mut steps = disk::disk_steps(cfg, ctx);
    steps.extend(base::base_steps(cfg, ctx));
    steps
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
