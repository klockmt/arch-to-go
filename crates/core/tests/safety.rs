use arch_to_go_core::safety::{
    DiskInfo, SafetyError, SystemInfo, check_target, confirm_matches, device_of_source,
    parse_disk_info,
};

struct Fake {
    info: Option<DiskInfo>,
    host: Option<Vec<String>>, // None = host disks cannot be listed
    mounted: bool,
}

impl Fake {
    fn usb(name: &str) -> Fake {
        Fake {
            info: Some(DiskInfo {
                name: name.into(),
                kind: "disk".into(),
                tran: "usb".into(),
            }),
            host: Some(vec!["nvme0n1".into()]),
            mounted: false,
        }
    }
}

impl SystemInfo for Fake {
    fn disk_info(&self, _dev: &str) -> Result<DiskInfo, SafetyError> {
        self.info.clone().ok_or(SafetyError::NotADevice("x".into()))
    }
    fn host_disks(&self) -> Result<Vec<String>, SafetyError> {
        self.host.clone().ok_or(SafetyError::HostDisksUnreadable)
    }
    fn has_mounted(&self, _dev: &str) -> Result<bool, SafetyError> {
        Ok(self.mounted)
    }
}

#[test]
fn accepts_unmounted_usb_disk() {
    check_target("/dev/sda", &Fake::usb("sda")).unwrap();
}

#[test]
fn rejects_non_disk() {
    let mut f = Fake::usb("sda1");
    f.info.as_mut().unwrap().kind = "part".into();
    assert!(check_target("/dev/sda1", &f).is_err());
}

#[test]
fn rejects_non_usb() {
    for tran in ["sata", "nvme", ""] {
        let mut f = Fake::usb("sdb");
        f.info.as_mut().unwrap().tran = tran.into();
        assert!(check_target("/dev/sdb", &f).is_err(), "{tran:?}");
    }
}

#[test]
fn rejects_host_disk() {
    let mut f = Fake::usb("sda");
    f.host = Some(vec!["nvme0n1".into(), "sda".into()]);
    assert!(matches!(
        check_target("/dev/sda", &f),
        Err(SafetyError::HostDisk(_))
    ));
}

#[test]
fn rejects_mounted_partitions() {
    let mut f = Fake::usb("sda");
    f.mounted = true;
    assert!(matches!(
        check_target("/dev/sda", &f),
        Err(SafetyError::Mounted(_))
    ));
}

#[test]
fn fails_closed_when_host_disks_unreadable() {
    let mut f = Fake::usb("sda");
    f.host = None;
    assert!(check_target("/dev/sda", &f).is_err());
}

#[test]
fn rejects_path_outside_dev() {
    for p in ["sda", "/tmp/sda", "/dev/../etc/passwd", "/dev/sda ", ""] {
        assert!(check_target(p, &Fake::usb("sda")).is_err(), "{p:?}");
    }
}

#[test]
fn confirm_requires_exact_path() {
    confirm_matches("/dev/sda", "/dev/sda").unwrap();
    for typed in ["/dev/sda ", "sda", "/dev/SDA", ""] {
        assert!(confirm_matches("/dev/sda", typed).is_err(), "{typed:?}");
    }
}

#[test]
fn btrfs_subvolume_suffix_is_stripped() {
    assert_eq!(device_of_source("/dev/mapper/x[/@]"), "/dev/mapper/x");
    assert_eq!(device_of_source("/dev/sda2"), "/dev/sda2");
}

#[test]
fn lsblk_line_is_parsed() {
    let d = parse_disk_info("sda disk usb\n").unwrap();
    assert_eq!(
        (d.name.as_str(), d.kind.as_str(), d.tran.as_str()),
        ("sda", "disk", "usb")
    );
    // an empty transport column leaves only two fields
    assert_eq!(parse_disk_info("loop0 loop\n").unwrap().tran, "");
}
