//! Desktop profile, network backend, zram, AUR helper and the user's extras.

use super::{Ctx, run, write};
use crate::config::{Config, NetworkBackend, Profile};
use crate::step::Step;

const HYPRLAND_CONF: &str = include_str!("../../assets/hyprland.conf");
const IWD_MAIN: &str = include_str!("../../assets/iwd-main.conf");
const DHCP_NETWORK: &str = include_str!("../../assets/20-dhcp.network");

fn chroot(desc: &str, argv: &[&str]) -> Step {
    let mut full = vec!["arch-chroot", "/mnt"];
    full.extend_from_slice(argv);
    run(desc, &full)
}

/// Packages and display-manager unit of a profile.
fn profile_parts(p: Profile) -> (&'static [&'static str], Option<&'static str>) {
    match p {
        Profile::Minimal => (&[], None),
        Profile::Hyprland => (
            &["hyprland", "kitty", "fish", "nautilus", "ttf-dejavu", "ly"],
            Some("ly@tty1.service"),
        ),
        Profile::Gnome => (
            &[
                "gnome-shell",
                "gdm",
                "nautilus",
                "gnome-console",
                "gnome-control-center",
                "ttf-dejavu",
            ],
            Some("gdm.service"),
        ),
        Profile::Kde => (
            &["plasma-desktop", "sddm", "konsole", "dolphin", "ttf-dejavu"],
            Some("sddm.service"),
        ),
    }
}

pub fn profile_steps(cfg: &Config, ctx: &Ctx) -> Vec<Step> {
    let admin = cfg.users.admin.as_str();
    let (desktop_pkgs, dm) = profile_parts(cfg.profile.name);

    let mut pkgs: Vec<&str> = desktop_pkgs.to_vec();
    if cfg.profile.audio {
        pkgs.extend(["pipewire", "pipewire-alsa", "pipewire-pulse", "wireplumber"]);
    }
    let mut units: Vec<&str> = dm.into_iter().collect();
    match cfg.network.backend {
        NetworkBackend::IwdNetworkd => {
            pkgs.push("iwd");
            units.extend(["iwd", "systemd-networkd", "systemd-resolved"]);
        }
        NetworkBackend::NetworkManager => {
            pkgs.push("networkmanager");
            units.push("NetworkManager.service");
        }
    }
    if cfg.portable.gpu_drivers {
        pkgs.extend([
            "mesa",
            "vulkan-radeon",
            "vulkan-intel",
            "intel-media-driver",
        ]);
    }
    if cfg.portable.firmware {
        pkgs.push("sof-firmware");
    }
    if cfg.disk.zram {
        pkgs.push("zram-generator");
    }
    if cfg.aur.paru {
        pkgs.extend(["base-devel", "git"]);
    }
    pkgs.extend(cfg.packages.iter().map(String::as_str));
    units.extend(cfg.services.iter().map(String::as_str));

    // each name once, first position wins
    let mut seen = Vec::new();
    pkgs.retain(|p| {
        !seen.contains(p) && {
            seen.push(*p);
            true
        }
    });

    let mut s = Vec::new();
    if !pkgs.is_empty() {
        let mut argv = vec!["pacman", "-S", "--noconfirm", "--needed"];
        argv.extend(&pkgs);
        s.push(chroot("install packages", &argv));
    }

    if cfg.profile.name == Profile::Hyprland {
        let layout = cfg.system.keymap.split('-').next().unwrap_or("us");
        let home = format!("/home/{admin}");
        s.push(chroot(
            "login shell fish",
            &["usermod", "-s", "/usr/bin/fish", admin],
        ));
        s.push(write(
            "hyprland config",
            &format!("/mnt{home}/.config/hypr/hyprland.conf"),
            &HYPRLAND_CONF.replace("@KB_LAYOUT@", layout),
            0o644,
        ));
        s.push(chroot(
            "own the user config",
            &[
                "chown",
                "-R",
                &format!("{admin}:{admin}"),
                &format!("{home}/.config"),
            ],
        ));
        // ly starts the session on tty1; getty@tty1 must not compete for it
        s.push(chroot(
            "free tty1",
            &["systemctl", "disable", "getty@tty1.service"],
        ));
    }

    if cfg.network.backend == NetworkBackend::IwdNetworkd {
        // iwd only handles Wi-Fi; systemd-networkd gets the address
        s.push(write(
            "iwd config",
            "/mnt/etc/iwd/main.conf",
            IWD_MAIN,
            0o644,
        ));
        s.push(write(
            "DHCP network",
            "/mnt/etc/systemd/network/20-dhcp.network",
            DHCP_NETWORK,
            0o644,
        ));
        s.push(chroot(
            "resolv.conf via resolved",
            &[
                "ln",
                "-sf",
                "/run/systemd/resolve/stub-resolv.conf",
                "/etc/resolv.conf",
            ],
        ));
    }

    if cfg.disk.zram {
        s.push(write(
            "zram config",
            "/mnt/etc/systemd/zram-generator.conf",
            "[zram0]\nzram-size = ram / 2\n",
            0o644,
        ));
    }

    if let (true, Some(pkg)) = (cfg.aur.paru, &ctx.paru_pkg) {
        s.push(run(
            "copy paru package",
            &["cp", pkg, "/mnt/root/paru.pkg.tar.zst"],
        ));
        s.push(chroot(
            "install paru",
            &["pacman", "-U", "--noconfirm", "/root/paru.pkg.tar.zst"],
        ));
        s.push(run(
            "remove paru package",
            &["rm", "-f", "/mnt/root/paru.pkg.tar.zst"],
        ));
    }

    if !units.is_empty() {
        let mut argv = vec!["systemctl", "enable"];
        argv.extend(&units);
        s.push(chroot("enable services", &argv));
    }
    s
}
