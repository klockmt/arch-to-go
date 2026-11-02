//! pacstrap, fstab, locale, hostname, initramfs and the admin account.

use super::{Ctx, run, write};
use crate::config::{Config, Fs, Kernel};
use crate::step::{Cmd, Secret, Step};

const PACMAN_CONF: &str = include_str!("../../assets/pacman.conf");
const SCRATCH: &str = "/mnt/.arch-to-go";
/// The same official mirrors as the scratch pacman.conf. pacstrap copies the host's
/// mirrorlist, which may point at another distro's mirror.
const MIRRORLIST: &str = "Server = https://geo.mirror.pkgbuild.com/$repo/os/$arch\nServer = https://fastly.mirror.pkgbuild.com/$repo/os/$arch\nServer = https://mirror.rackspace.com/archlinux/$repo/os/$arch\n";

fn chroot(desc: &str, argv: &[&str]) -> Step {
    let mut full = vec!["arch-chroot", "/mnt"];
    full.extend_from_slice(argv);
    run(desc, &full)
}

fn preset(k: Kernel) -> String {
    let n = k.name();
    format!(
        "ALL_config=/etc/mkinitcpio.conf\nALL_kver=/boot/vmlinuz-{n}\nPRESETS=(default)\ndefault_image=/boot/initramfs-{n}.img\n"
    )
}

/// The kernel package puts vmlinuz under /usr/lib/modules/<ver>/ with a `pkgbase`
/// file naming the package; mkinitcpio wants it at /boot/vmlinuz-<pkg>. `k` is an
/// enum name, never user text.
fn copy_kernel_script(k: Kernel) -> String {
    let n = k.name();
    format!(
        "for d in /usr/lib/modules/*/; do [ \"$(cat \"${{d}}pkgbase\" 2>/dev/null)\" = {n} ] && cp \"${{d}}vmlinuz\" /boot/vmlinuz-{n}; done; test -f /boot/vmlinuz-{n}"
    )
}

pub fn base_steps(cfg: &Config, ctx: &Ctx) -> Vec<Step> {
    let sys = &cfg.system;
    let admin = cfg.users.admin.as_str();

    // pacstrap -C: the host pacman.conf may carry other distros' repositories
    // that must not reach the installed system
    let conf = format!("{SCRATCH}/pacman.conf");
    let mut pkgs: Vec<&str> = vec!["base"];
    pkgs.extend(cfg.boot.kernels.iter().map(|k| k.name()));
    if cfg.portable.firmware {
        pkgs.extend(["linux-firmware", "intel-ucode", "amd-ucode"]);
    }
    match cfg.disk.fs {
        Fs::Btrfs => pkgs.push("btrfs-progs"),
        Fs::Xfs => pkgs.push("xfsprogs"),
        Fs::Ext4 => {}
    }
    pkgs.push("sudo");
    let mut pacstrap = vec!["pacstrap", "-C", &conf, "-K", "/mnt"];
    pacstrap.extend(pkgs);

    let mut locales = String::from("en_US.UTF-8 UTF-8\n");
    if sys.locale != "en_US.UTF-8" {
        locales.push_str(&format!("{} UTF-8\n", sys.locale));
    }
    let modules = if cfg.disk.fs == Fs::Btrfs {
        "btrfs"
    } else {
        ""
    };
    let zone = format!("/usr/share/zoneinfo/{}", sys.timezone);

    let mut s = vec![
        write("scratch pacman.conf", &conf, PACMAN_CONF, 0o644),
        run("install base system", &pacstrap),
        run("remove scratch pacman.conf", &["rm", "-rf", SCRATCH]),
        write(
            "official mirrorlist",
            "/mnt/etc/pacman.d/mirrorlist",
            MIRRORLIST,
            0o644,
        ),
        // the redirect needs a shell; the command line has no config values
        run(
            "write fstab (by UUID)",
            &["sh", "-c", "genfstab -U /mnt >> /mnt/etc/fstab"],
        ),
        chroot("set timezone", &["ln", "-sf", &zone, "/etc/localtime"]),
        chroot("sync hardware clock", &["hwclock", "--systohc"]),
        write("locales", "/mnt/etc/locale.gen", &locales, 0o644),
        chroot("generate locales", &["locale-gen"]),
        write(
            "locale",
            "/mnt/etc/locale.conf",
            &format!("LANG={}\n", sys.locale),
            0o644,
        ),
        write(
            "hostname",
            "/mnt/etc/hostname",
            &format!("{}\n", sys.hostname),
            0o644,
        ),
        write(
            "console keymap",
            "/mnt/etc/vconsole.conf",
            &format!("KEYMAP={}\n", sys.keymap),
            0o644,
        ),
        // no autodetect: the initramfs must boot on machines other than this one
        write(
            "initramfs hooks",
            "/mnt/etc/mkinitcpio.conf.d/arch-to-go.conf",
            &format!(
                "MODULES=({modules})\nHOOKS=(base udev block keyboard keymap filesystems fsck)\n"
            ),
            0o644,
        ),
    ];
    for &k in &cfg.boot.kernels {
        s.push(write(
            "initramfs preset",
            &format!("/mnt/etc/mkinitcpio.d/{}.preset", k.name()),
            &preset(k),
            0o644,
        ));
        s.push(chroot(
            "copy kernel to /boot",
            &["sh", "-c", &copy_kernel_script(k)],
        ));
    }
    s.push(chroot("build initramfs", &["mkinitcpio", "-P"]));
    s.push(chroot(
        "create admin user",
        &["useradd", "-m", "-G", "wheel", admin],
    ));
    s.push(write(
        "sudo for wheel",
        "/mnt/etc/sudoers.d/10-wheel",
        "%wheel ALL=(ALL:ALL) ALL\n",
        0o440,
    ));
    // root gets the admin password too (see README); stdin keeps it out of argv
    for user in ["root", admin] {
        s.push(Step::Run {
            desc: format!("set password of {user}"),
            cmd: Cmd::new(&["arch-chroot", "/mnt", "chpasswd"]).with_stdin(Secret::new(format!(
                "{user}:{}",
                ctx.admin_password.expose()
            ))),
        });
    }
    s
}
