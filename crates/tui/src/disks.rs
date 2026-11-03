//! USB disks the user can pick, from `lsblk`. Listing only: the CLI re-checks the target.

use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disk {
    /// The device path, e.g. `/dev/sda`.
    pub name: String,
    pub size: String,
    pub model: String,
}

/// Value of `KEY="..."` in an `lsblk -P` line (values may contain spaces).
fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let start = line.find(&format!("{key}=\""))? + key.len() + 2;
    let rest = &line[start..];
    Some(&rest[..rest.find('"')?])
}

/// Parses `lsblk -dnP -o NAME,TYPE,TRAN,SIZE,MODEL`; keeps whole USB disks only, plus loop
/// devices when `allow_loop` is set (the CLI's `ARCH_TO_GO_ALLOW_LOOP=1`, for tests).
pub fn parse_lsblk(out: &str, allow_loop: bool) -> Vec<Disk> {
    out.lines()
        .filter_map(|l| {
            let name = field(l, "NAME").filter(|n| !n.is_empty())?;
            let (kind, tran) = (field(l, "TYPE")?, field(l, "TRAN")?);
            ((kind == "disk" && tran == "usb") || (allow_loop && kind == "loop")).then(|| Disk {
                name: format!("/dev/{name}"),
                size: field(l, "SIZE").unwrap_or("").to_owned(),
                model: field(l, "MODEL").unwrap_or("").trim().to_owned(),
            })
        })
        .collect()
}

pub fn list_usb_disks() -> Result<Vec<Disk>, String> {
    let out = Command::new("lsblk")
        .args(["-dnP", "-o", "NAME,TYPE,TRAN,SIZE,MODEL"])
        .output()
        .map_err(|e| format!("cannot run lsblk: {e}"))?;
    if !out.status.success() {
        return Err("lsblk failed".into());
    }
    let allow_loop = std::env::var("ARCH_TO_GO_ALLOW_LOOP").is_ok_and(|v| v == "1");
    Ok(parse_lsblk(
        &String::from_utf8_lossy(&out.stdout),
        allow_loop,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUT: &str = r#"NAME="nvme0n1" TYPE="disk" TRAN="nvme" SIZE="476,9G" MODEL="Samsung SSD 980"
NAME="sdb" TYPE="disk" TRAN="sata" SIZE="931,5G" MODEL="WDC WD10"
NAME="sda" TYPE="disk" TRAN="usb" SIZE="111,8G" MODEL="KINGSTON SA400S37120G"
NAME="sda1" TYPE="part" TRAN="usb" SIZE="1G" MODEL=""
"#;

    #[test]
    fn usb_disks_only() {
        let d = parse_lsblk(OUT, false);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].name, "/dev/sda");
        assert_eq!(d[0].size, "111,8G");
        assert_eq!(d[0].model, "KINGSTON SA400S37120G");
    }

    #[test]
    fn loop_devices_are_listed_only_when_allowed() {
        let out = format!("{OUT}NAME=\"loop0\" TYPE=\"loop\" TRAN=\"\" SIZE=\"8G\" MODEL=\"\"\n");
        assert_eq!(parse_lsblk(&out, false).len(), 1);
        let with = parse_lsblk(&out, true);
        assert_eq!(with.len(), 2, "{with:?}");
        assert!(with.iter().any(|d| d.name == "/dev/loop0"));
        // the flag opens loop devices only, never SATA or NVMe disks
        assert!(
            with.iter()
                .all(|d| d.name != "/dev/sdb" && d.name != "/dev/nvme0n1")
        );
    }

    #[test]
    fn empty_output_gives_no_disks() {
        assert!(parse_lsblk("", false).is_empty());
    }

    #[test]
    fn garbage_lines_are_ignored() {
        assert!(parse_lsblk("not lsblk\nNAME=\nTYPE=\"disk\"\n\n", false).is_empty());
    }
}
