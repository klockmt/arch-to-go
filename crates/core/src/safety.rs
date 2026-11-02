//! Refuses any target that could be a system disk. Every check fails closed:
//! if the system cannot be inspected, the install does not start.

use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskInfo {
    pub name: String,
    pub kind: String,
    pub tran: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SafetyError {
    #[error("{0:?} is not a plain /dev/ path")]
    BadPath(String),
    #[error("{0} is not a block device")]
    NotADevice(String),
    #[error("{0} is not a whole disk (it is '{1}')")]
    NotWholeDisk(String, String),
    #[error("{0} is not USB (transport '{1}'); refusing for safety")]
    NotUsb(String, String),
    #[error("{0} backs the running system; refusing")]
    HostDisk(String),
    #[error("{0} has mounted partitions; unmount them first")]
    Mounted(String),
    #[error("could not list the disks of the running system")]
    HostDisksUnreadable,
    #[error("`{0}` failed")]
    Command(String),
    #[error("confirmation did not match; nothing was done")]
    ConfirmMismatch,
}

/// What the safety checks need to know about the machine; faked in tests.
pub trait SystemInfo {
    fn disk_info(&self, dev: &str) -> Result<DiskInfo, SafetyError>;
    /// Names (`sda`, `nvme0n1`) of the disks behind every mounted filesystem.
    fn host_disks(&self) -> Result<Vec<String>, SafetyError>;
    fn has_mounted(&self, dev: &str) -> Result<bool, SafetyError>;
}

pub fn check_target(dev: &str, sys: &dyn SystemInfo) -> Result<(), SafetyError> {
    let plain = dev.strip_prefix("/dev/").is_some_and(|n| {
        !n.is_empty()
            && n.chars()
                .all(|c| c.is_ascii_alphanumeric() || "_-".contains(c))
    });
    if !plain {
        return Err(SafetyError::BadPath(dev.into()));
    }
    let info = sys.disk_info(dev)?;
    if info.kind != "disk" {
        return Err(SafetyError::NotWholeDisk(dev.into(), info.kind));
    }
    if info.tran != "usb" {
        return Err(SafetyError::NotUsb(dev.into(), info.tran));
    }
    if sys.host_disks()?.iter().any(|d| *d == info.name) {
        return Err(SafetyError::HostDisk(dev.into()));
    }
    if sys.has_mounted(dev)? {
        return Err(SafetyError::Mounted(dev.into()));
    }
    Ok(())
}

pub fn confirm_matches(dev: &str, typed: &str) -> Result<(), SafetyError> {
    if dev == typed {
        Ok(())
    } else {
        Err(SafetyError::ConfirmMismatch)
    }
}

/// `findmnt` shows a btrfs subvolume as `/dev/x[/@]`; keep only the device.
pub fn device_of_source(src: &str) -> &str {
    src.split('[').next().unwrap_or(src)
}

/// Parses `lsblk -dno NAME,TYPE,TRAN`; TRAN is empty for loop devices.
pub fn parse_disk_info(out: &str) -> Result<DiskInfo, SafetyError> {
    let mut f = out.split_whitespace();
    match (f.next(), f.next()) {
        (Some(name), Some(kind)) => Ok(DiskInfo {
            name: name.into(),
            kind: kind.into(),
            tran: f.next().unwrap_or("").into(),
        }),
        _ => Err(SafetyError::NotADevice(out.trim().into())),
    }
}

/// The real machine, inspected with `lsblk` and `findmnt`.
pub struct HostSystem;

fn output(prog: &str, args: &[&str]) -> Result<String, SafetyError> {
    let shown = format!("{prog} {}", args.join(" "));
    let out = Command::new(prog)
        .args(args)
        .output()
        .map_err(|_| SafetyError::Command(shown.clone()))?;
    if !out.status.success() {
        return Err(SafetyError::Command(shown));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

impl SystemInfo for HostSystem {
    fn disk_info(&self, dev: &str) -> Result<DiskInfo, SafetyError> {
        parse_disk_info(
            &output("lsblk", &["-dno", "NAME,TYPE,TRAN", dev])
                .map_err(|_| SafetyError::NotADevice(dev.into()))?,
        )
    }

    fn host_disks(&self) -> Result<Vec<String>, SafetyError> {
        let sources =
            output("findmnt", &["-rno", "SOURCE"]).map_err(|_| SafetyError::HostDisksUnreadable)?;
        let mut disks = Vec::new();
        for src in sources.lines().map(device_of_source) {
            if !src.starts_with("/dev/") {
                continue;
            }
            let tree = output("lsblk", &["-srno", "NAME,TYPE", src])
                .map_err(|_| SafetyError::HostDisksUnreadable)?;
            for line in tree.lines() {
                let mut f = line.split_whitespace();
                if let (Some(name), Some("disk")) = (f.next(), f.next()) {
                    disks.push(name.to_owned());
                }
            }
        }
        Ok(disks)
    }

    fn has_mounted(&self, dev: &str) -> Result<bool, SafetyError> {
        Ok(output("lsblk", &["-rno", "MOUNTPOINTS", dev])?
            .lines()
            .any(|l| !l.trim().is_empty()))
    }
}
