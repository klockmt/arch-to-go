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

/// Parses `lsblk -dnP -o NAME,TYPE,TRAN,SIZE,MODEL`; keeps whole USB disks only.
pub fn parse_lsblk(out: &str) -> Vec<Disk> {
    out.lines()
        .filter_map(|l| {
            let name = field(l, "NAME").filter(|n| !n.is_empty())?;
            (field(l, "TYPE")? == "disk" && field(l, "TRAN")? == "usb").then(|| Disk {
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
    Ok(parse_lsblk(&String::from_utf8_lossy(&out.stdout)))
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
        let d = parse_lsblk(OUT);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].name, "/dev/sda");
        assert_eq!(d[0].size, "111,8G");
        assert_eq!(d[0].model, "KINGSTON SA400S37120G");
    }

    #[test]
    fn empty_output_gives_no_disks() {
        assert!(parse_lsblk("").is_empty());
    }

    #[test]
    fn garbage_lines_are_ignored() {
        assert!(parse_lsblk("not lsblk\nNAME=\nTYPE=\"disk\"\n\n").is_empty());
    }
}
