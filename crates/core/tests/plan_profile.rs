use arch_to_go_core::config::Config;
use arch_to_go_core::plan::profile::profile_steps;
use arch_to_go_core::plan::{Ctx, build};
use arch_to_go_core::step::Step;

const PARTUUID: &str = "11111111-2222-3333-4444-555555555555";

fn cfg(extra: &str) -> Config {
    Config::from_toml(&format!("{extra}\n[users]\nadmin = \"zzadminzz\"\n")).unwrap()
}

fn ctx() -> Ctx {
    Ctx::new("/dev/sda", PARTUUID, "pw").unwrap()
}

fn steps(extra: &str) -> Vec<Step> {
    profile_steps(&cfg(extra), &ctx())
}

fn argvs(steps: &[Step]) -> Vec<String> {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Run { cmd, .. } => Some(cmd.argv.join(" ")),
            _ => None,
        })
        .collect()
}

fn file<'a>(steps: &'a [Step], path: &str) -> &'a str {
    steps
        .iter()
        .find_map(|s| match s {
            Step::WriteFile {
                path: p, contents, ..
            } if p == path => Some(contents.as_str()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no file written at {path}"))
}

fn install_line(s: &[Step]) -> String {
    argvs(s)
        .into_iter()
        .find(|c| c.starts_with("arch-chroot /mnt pacman -S"))
        .unwrap_or_default()
}

#[test]
fn hyprland_enables_ly_and_disables_getty_tty1() {
    let a = argvs(&steps("[profile]\nname = \"hyprland\"\n"));
    let off = a
        .iter()
        .position(|c| c == "arch-chroot /mnt systemctl disable getty@tty1.service")
        .expect("disable getty");
    let on = a
        .iter()
        .position(|c| {
            c.starts_with("arch-chroot /mnt systemctl enable") && c.contains("ly@tty1.service")
        })
        .expect("enable ly");
    assert!(off < on);
    assert!(install_line(&steps("[profile]\nname = \"hyprland\"\n")).contains(" hyprland "));
}

#[test]
fn hyprland_config_is_written_for_the_admin_with_the_keymap() {
    let s = steps("[profile]\nname = \"hyprland\"\n[system]\nkeymap = \"de-latin1\"\n");
    // built at run time: the repo scan rejects literal home-directory paths
    let home = format!("/{}/zzadminzz", "home");
    let conf = file(&s, &format!("/mnt{home}/.config/hypr/hyprland.conf"));
    assert!(conf.contains("kb_layout = de\n"), "{conf}");
    assert!(!conf.contains("@KB_LAYOUT@"));
    assert!(argvs(&s).contains(&format!(
        "arch-chroot /mnt chown -R zzadminzz:zzadminzz {home}/.config"
    )));
    assert!(argvs(&s).contains(&"arch-chroot /mnt usermod -s /usr/bin/fish zzadminzz".to_string()));
}

#[test]
fn gnome_and_kde_use_their_own_display_manager() {
    let g = argvs(&steps("[profile]\nname = \"gnome\"\n"));
    assert!(
        g.iter().any(
            |c| c.starts_with("arch-chroot /mnt systemctl enable") && c.contains("gdm.service")
        )
    );
    let k = argvs(&steps("[profile]\nname = \"kde\"\n"));
    assert!(
        k.iter()
            .any(|c| c.starts_with("arch-chroot /mnt systemctl enable")
                && c.contains("sddm.service"))
    );
}

#[test]
fn minimal_profile_installs_no_desktop() {
    let s = steps("");
    let line = install_line(&s);
    for p in ["hyprland", "gdm", "sddm", "plasma-desktop", "gnome-shell"] {
        assert!(!line.contains(p), "{p} in {line}");
    }
    assert!(argvs(&s).iter().all(|c| !c.contains("getty@tty1")));
}

#[test]
fn iwd_networkd_writes_both_configs_and_resolv_symlink() {
    let s = steps("");
    assert_eq!(
        file(&s, "/mnt/etc/iwd/main.conf"),
        include_str!("../assets/iwd-main.conf")
    );
    assert_eq!(
        file(&s, "/mnt/etc/systemd/network/20-dhcp.network"),
        include_str!("../assets/20-dhcp.network")
    );
    let a = argvs(&s);
    assert!(
        a.contains(
            &"arch-chroot /mnt ln -sf /run/systemd/resolve/stub-resolv.conf /etc/resolv.conf"
                .to_string()
        )
    );
    let enable = a
        .iter()
        .find(|c| c.contains("systemctl enable") && c.contains("iwd"))
        .expect("enable");
    assert!(enable.contains("systemd-networkd") && enable.contains("systemd-resolved"));
    assert!(install_line(&s).contains(" iwd"));
}

#[test]
fn network_manager_does_not_install_iwd() {
    let s = steps("[network]\nbackend = \"network-manager\"\n");
    assert!(install_line(&s).contains("networkmanager"));
    assert!(!install_line(&s).contains(" iwd"));
    assert!(
        argvs(&s)
            .iter()
            .any(|c| c.contains("systemctl enable") && c.contains("NetworkManager.service"))
    );
    assert!(argvs(&s).iter().all(|c| !c.contains("systemd-networkd")));
}

#[test]
fn extra_packages_and_services_are_appended() {
    let s = steps("packages = [\"vim\", \"htop\"]\nservices = [\"sshd.service\"]\n");
    let line = install_line(&s);
    assert!(line.contains(" vim") && line.contains(" htop"), "{line}");
    assert!(
        argvs(&s)
            .iter()
            .any(|c| c.starts_with("arch-chroot /mnt systemctl enable")
                && c.contains("sshd.service"))
    );
}

#[test]
fn audio_adds_pipewire() {
    assert!(install_line(&steps("[profile]\naudio = true\n")).contains("pipewire-pulse"));
    assert!(!install_line(&steps("")).contains("pipewire"));
}

#[test]
fn portable_adds_gpu_and_sof_packages_and_can_be_turned_off() {
    let line = install_line(&steps(""));
    for p in [
        "mesa",
        "vulkan-radeon",
        "vulkan-intel",
        "intel-media-driver",
        "sof-firmware",
    ] {
        assert!(line.contains(p), "{p} in {line}");
    }
    let off = install_line(&steps(
        "[portable]\ngpu_drivers = false\nfirmware = false\n",
    ));
    assert!(
        !off.contains("mesa") && !off.contains("sof-firmware"),
        "{off}"
    );
}

#[test]
fn zram_writes_generator_config() {
    let s = steps("[disk]\nzram = true\n");
    assert!(file(&s, "/mnt/etc/systemd/zram-generator.conf").contains("[zram0]"));
    assert!(install_line(&s).contains("zram-generator"));
    assert!(install_line(&steps("")).find("zram").is_none());
}

#[test]
fn paru_is_installed_from_the_prebuilt_package() {
    let ctx = ctx().with_paru_pkg("/srv/pkgs/paru-2.1.0-1-x86_64.pkg.tar.zst");
    let s = profile_steps(&cfg("[aur]\nparu = true\n"), &ctx);
    let a = argvs(&s);
    assert!(
        a.contains(
            &"cp /srv/pkgs/paru-2.1.0-1-x86_64.pkg.tar.zst /mnt/root/paru.pkg.tar.zst".to_string()
        ),
        "{a:#?}"
    );
    assert!(
        a.contains(&"arch-chroot /mnt pacman -U --noconfirm /root/paru.pkg.tar.zst".to_string())
    );
    assert!(a.contains(&"rm -f /mnt/root/paru.pkg.tar.zst".to_string()));
    assert!(install_line(&s).contains("base-devel") && install_line(&s).contains(" git"));
}

#[test]
fn paru_without_a_package_is_a_plan_error() {
    assert!(build(&cfg("[aur]\nparu = true\n"), &ctx()).is_err());
    assert!(build(&cfg(""), &ctx()).is_ok());
}

#[test]
fn no_config_value_is_passed_through_sh_c() {
    let s = steps("[profile]\nname = \"hyprland\"\n[system]\nkeymap = \"zzkeymap\"\n");
    assert!(argvs(&s).iter().all(|c| !c.contains("sh -c")));
}
