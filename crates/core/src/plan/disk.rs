//! Partition, format and mount the target at `/mnt`.

use super::{Ctx, run};
use crate::config::{Config, Fs};
use crate::step::Step;

/// `part("/dev/sda", 1)` is `/dev/sda1`; `part("/dev/nvme0n1", 1)` is `/dev/nvme0n1p1`.
pub fn part(disk: &str, n: u8) -> String {
    if disk.ends_with(|c: char| c.is_ascii_digit()) {
        format!("{disk}p{n}")
    } else {
        format!("{disk}{n}")
    }
}

/// Where a btrfs subvolume is mounted under `/mnt`.
fn subvol_mountpoint(sv: &str) -> String {
    match sv {
        "@home" => "/home".into(),
        "@log" => "/var/log".into(),
        "@pkg" => "/var/cache/pacman/pkg".into(),
        other => format!("/{}", other.trim_start_matches('@')),
    }
}

pub fn disk_steps(cfg: &Config, ctx: &Ctx) -> Vec<Step> {
    let d = ctx.disk.as_str();
    let (esp, root) = (part(d, 1), part(d, 2));
    let disk = &cfg.disk;
    let mut s = vec![
        run("wipe signatures", &["wipefs", "--all", "--force", d]),
        run("clear partition table", &["sgdisk", "--zap-all", d]),
        run(
            "create ESP",
            &[
                "sgdisk",
                &format!("-n1:0:+{}", disk.esp_size),
                "-t1:ef00",
                "-c1:ESP",
                d,
            ],
        ),
        run(
            "create root partition",
            &[
                "sgdisk",
                "-n2:0:0",
                "-t2:8300",
                "-c2:archroot",
                &format!("-u2:{}", ctx.root_partuuid),
                d,
            ],
        ),
        run("reread partition table", &["partprobe", d]),
        run("wait for udev", &["udevadm", "settle"]),
        run("format ESP", &["mkfs.fat", "-F32", "-n", "ESP", &esp]),
    ];
    s.push(match disk.fs {
        Fs::Btrfs => run(
            "format root",
            &["mkfs.btrfs", "-f", "-L", "archroot", &root],
        ),
        Fs::Ext4 => run("format root", &["mkfs.ext4", "-F", "-L", "archroot", &root]),
        Fs::Xfs => run("format root", &["mkfs.xfs", "-f", "-L", "archroot", &root]),
    });

    if disk.fs == Fs::Btrfs {
        let mut opts = String::from("noatime,ssd,discard=async");
        if disk.nodatacow {
            opts.push_str(",nodatacow");
        }
        let subvols = disk.effective_subvolumes();
        s.push(run("mount top level", &["mount", &root, "/mnt"]));
        for sv in &subvols {
            s.push(run(
                "create subvolume",
                &["btrfs", "subvolume", "create", &format!("/mnt/{sv}")],
            ));
        }
        s.push(run("unmount top level", &["umount", "/mnt"]));
        s.push(run(
            "mount root subvolume",
            &["mount", "-o", &format!("subvol=@,{opts}"), &root, "/mnt"],
        ));
        for sv in subvols.iter().filter(|v| *v != "@") {
            let at = format!("/mnt{}", subvol_mountpoint(sv));
            s.push(run("create mountpoint", &["mkdir", "-p", &at]));
            s.push(run(
                "mount subvolume",
                &["mount", "-o", &format!("subvol={sv},{opts}"), &root, &at],
            ));
        }
    } else {
        s.push(run(
            "mount root",
            &["mount", "-o", "noatime", &root, "/mnt"],
        ));
    }
    s.push(run("create /boot", &["mkdir", "-p", "/mnt/boot"]));
    s.push(run("mount ESP", &["mount", &esp, "/mnt/boot"]));
    s
}
