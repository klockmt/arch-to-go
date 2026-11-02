use arch_to_go_core::config::Config;
use arch_to_go_core::plan::Ctx;
use arch_to_go_core::plan::disk::{disk_steps, part};
use arch_to_go_core::step::Step;

const PARTUUID: &str = "11111111-2222-3333-4444-555555555555";

fn ctx(disk: &str) -> Ctx {
    Ctx::new(disk, PARTUUID, "x").unwrap()
}

fn cfg(extra: &str) -> Config {
    Config::from_toml(&format!("{extra}\n[users]\nadmin = \"tester\"\n")).unwrap()
}

/// Every command as one line, in order.
fn lines(steps: &[Step]) -> Vec<String> {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Run { cmd, .. } => Some(cmd.argv.join(" ")),
            _ => None,
        })
        .collect()
}

fn pos(lines: &[String], needle: &str) -> usize {
    lines
        .iter()
        .position(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no step contains {needle:?} in {lines:#?}"))
}

#[test]
fn part_adds_p_for_digit_suffix() {
    assert_eq!(part("/dev/sda", 1), "/dev/sda1");
    assert_eq!(part("/dev/nvme0n1", 1), "/dev/nvme0n1p1");
    assert_eq!(part("/dev/mmcblk0", 2), "/dev/mmcblk0p2");
    assert_eq!(part("/dev/loop0", 2), "/dev/loop0p2");
}

#[test]
fn disk_steps_order() {
    let l = lines(&disk_steps(&cfg(""), &ctx("/dev/sda")));
    let order = [
        "wipefs --all --force /dev/sda",
        "sgdisk --zap-all /dev/sda",
        "-n1:0:+1G -t1:ef00",
        "-n2:0:0 -t2:8300",
        "partprobe /dev/sda",
        "udevadm settle",
        "mkfs.fat -F32 -n ESP /dev/sda1",
        "mkfs.btrfs -f -L archroot /dev/sda2",
        "btrfs subvolume create /mnt/@home",
        "mount -o subvol=@,noatime,ssd,discard=async /dev/sda2 /mnt",
        "mount -o subvol=@home,noatime,ssd,discard=async /dev/sda2 /mnt/home",
        "mount -o subvol=@log,noatime,ssd,discard=async /dev/sda2 /mnt/var/log",
        "mount -o subvol=@pkg,noatime,ssd,discard=async /dev/sda2 /mnt/var/cache/pacman/pkg",
        "mount /dev/sda1 /mnt/boot",
    ];
    let idx: Vec<usize> = order.iter().map(|n| pos(&l, n)).collect();
    assert!(idx.windows(2).all(|w| w[0] < w[1]), "{idx:?} in {l:#?}");
}

#[test]
fn root_partition_gets_the_planned_partuuid() {
    let l = lines(&disk_steps(&cfg(""), &ctx("/dev/sda")));
    assert!(
        l.iter().any(|c| c.contains(&format!("-u2:{PARTUUID}"))),
        "{l:#?}"
    );
}

#[test]
fn nvme_uses_p_suffix_everywhere() {
    let l = lines(&disk_steps(&cfg(""), &ctx("/dev/nvme0n1")));
    assert!(
        l.iter().any(|c| c.ends_with("/dev/nvme0n1p1 /mnt/boot")),
        "{l:#?}"
    );
    assert!(l.iter().all(|c| !c.contains("/dev/nvme0n12")), "{l:#?}");
}

#[test]
fn esp_size_comes_from_config() {
    let l = lines(&disk_steps(
        &cfg("[disk]\nesp_size = \"512M\"\n"),
        &ctx("/dev/sda"),
    ));
    assert!(l.iter().any(|c| c.contains("-n1:0:+512M")), "{l:#?}");
}

#[test]
fn ext4_has_no_subvolumes() {
    let l = lines(&disk_steps(
        &cfg("[disk]\nfs = \"ext4\"\n"),
        &ctx("/dev/sda"),
    ));
    assert!(
        l.iter().any(|c| c == "mkfs.ext4 -F -L archroot /dev/sda2"),
        "{l:#?}"
    );
    assert!(
        l.iter()
            .all(|c| !c.contains("btrfs") && !c.contains("subvol")),
        "{l:#?}"
    );
    assert!(l.iter().any(|c| c.ends_with("/dev/sda2 /mnt")), "{l:#?}");
}

#[test]
fn nodatacow_only_when_requested() {
    let off = lines(&disk_steps(&cfg(""), &ctx("/dev/sda")));
    assert!(off.iter().all(|c| !c.contains("nodatacow")));
    let on = lines(&disk_steps(
        &cfg("[disk]\nnodatacow = true\n"),
        &ctx("/dev/sda"),
    ));
    assert!(
        on.iter()
            .any(|c| c.contains("noatime,ssd,discard=async,nodatacow"))
    );
}

#[test]
fn custom_subvolume_is_mounted_under_its_name() {
    let c = cfg("[disk]\nsubvolumes = [\"@\", \"@srv\"]\n");
    let l = lines(&disk_steps(&c, &ctx("/dev/sda")));
    assert!(
        l.iter().any(|c| c.ends_with("/dev/sda2 /mnt/srv")),
        "{l:#?}"
    );
}
