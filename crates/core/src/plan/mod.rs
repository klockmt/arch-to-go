//! Builds the ordered install steps from a validated `Config`. Pure: no I/O.

pub mod disk;

use crate::config::Config;
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

pub fn build(cfg: &Config, ctx: &Ctx) -> Vec<Step> {
    disk::disk_steps(cfg, ctx)
}

/// A command step.
pub(crate) fn run(desc: &str, argv: &[&str]) -> Step {
    Step::Run {
        desc: desc.into(),
        cmd: Cmd::new(argv),
    }
}
