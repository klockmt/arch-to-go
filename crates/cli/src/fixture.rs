//! A fake machine for tests, read from a directory of canned `lsblk`/`findmnt`
//! output. Debug builds only, and the CLI refuses it for a real run.

use arch_to_go_core::safety::{DiskInfo, SafetyError, SystemInfo, parse_disk_info};
use std::cell::Cell;
use std::path::PathBuf;

/// `disk_info.2`, when present, is what every inspection after the first sees: it
/// simulates the disk changing between the check and the wipe.
pub struct FixtureSystem(pub PathBuf, pub Cell<u32>);

impl SystemInfo for FixtureSystem {
    fn disk_info(&self, dev: &str) -> Result<DiskInfo, SafetyError> {
        let n = self.1.get();
        self.1.set(n + 1);
        let later = self.0.join("disk_info.2");
        let file = if n >= 1 && later.exists() {
            later
        } else {
            self.0.join("disk_info")
        };
        let s = std::fs::read_to_string(file).map_err(|_| SafetyError::NotADevice(dev.into()))?;
        parse_disk_info(&s)
    }

    fn host_disks(&self) -> Result<Vec<String>, SafetyError> {
        let s = std::fs::read_to_string(self.0.join("host_disks"))
            .map_err(|_| SafetyError::HostDisksUnreadable)?;
        Ok(s.lines().map(str::to_owned).collect())
    }

    fn has_mounted(&self, _dev: &str) -> Result<bool, SafetyError> {
        Ok(self.0.join("mounted").exists())
    }

    fn shares_filesystem_with_host(&self, _dev: &str) -> Result<bool, SafetyError> {
        Ok(self.0.join("shares_fs").exists())
    }
}
