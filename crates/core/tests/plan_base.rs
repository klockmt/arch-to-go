use arch_to_go_core::config::Config;
use arch_to_go_core::plan::Ctx;
use arch_to_go_core::plan::base::base_steps;
use arch_to_go_core::step::Step;

const PARTUUID: &str = "11111111-2222-3333-4444-555555555555";

fn ctx(password: &str) -> Ctx {
    Ctx::new("/dev/sda", PARTUUID, password).unwrap()
}

fn cfg(extra: &str) -> Config {
    Config::from_toml(&format!("{extra}\n[users]\nadmin = \"zzadminzz\"\n")).unwrap()
}

fn argvs(steps: &[Step]) -> Vec<Vec<String>> {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Run { cmd, .. } => Some(cmd.argv.clone()),
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

#[test]
fn pacstrap_uses_repo_pacman_conf() {
    let steps = base_steps(&cfg(""), &ctx("pw"));
    let conf = "/mnt/.arch-to-go/pacman.conf";
    assert_eq!(file(&steps, conf), include_str!("../assets/pacman.conf"));
    let pacstrap = argvs(&steps)
        .into_iter()
        .find(|a| a[0] == "pacstrap")
        .expect("pacstrap step");
    assert_eq!(&pacstrap[1..5], ["-C", conf, "-K", "/mnt"]);
    for pkg in ["base", "linux", "linux-firmware", "btrfs-progs", "sudo"] {
        assert!(pacstrap.iter().any(|p| p == pkg), "{pkg} in {pacstrap:?}");
    }
    // the scratch config is removed from the target afterwards
    assert!(
        argvs(&steps)
            .iter()
            .any(|a| a == &["rm", "-rf", "/mnt/.arch-to-go"])
    );
}

#[test]
fn kernels_and_fs_tools_follow_config() {
    let steps = base_steps(
        &cfg("[boot]\nkernels = [\"linux\", \"linux-lts\"]\n[disk]\nfs = \"xfs\"\n"),
        &ctx("pw"),
    );
    let pacstrap = argvs(&steps)
        .into_iter()
        .find(|a| a[0] == "pacstrap")
        .unwrap();
    assert!(pacstrap.contains(&"linux-lts".to_string()));
    assert!(pacstrap.contains(&"xfsprogs".to_string()));
    assert!(!pacstrap.contains(&"btrfs-progs".to_string()));
}

#[test]
fn portable_firmware_adds_ucode_and_can_be_turned_off() {
    let on = argvs(&base_steps(&cfg(""), &ctx("pw")));
    let p = on.iter().find(|a| a[0] == "pacstrap").unwrap();
    for pkg in ["intel-ucode", "amd-ucode"] {
        assert!(p.contains(&pkg.to_string()), "{pkg}");
    }
    let off = argvs(&base_steps(
        &cfg("[portable]\nfirmware = false\n"),
        &ctx("pw"),
    ));
    let p = off.iter().find(|a| a[0] == "pacstrap").unwrap();
    assert!(!p.contains(&"linux-firmware".to_string()));
    assert!(!p.contains(&"intel-ucode".to_string()));
}

#[test]
fn hooks_have_no_autodetect() {
    let steps = base_steps(&cfg(""), &ctx("pw"));
    let conf = file(&steps, "/mnt/etc/mkinitcpio.conf.d/arch-to-go.conf");
    assert!(
        conf.contains("HOOKS=(base udev block keyboard keymap filesystems fsck)"),
        "{conf}"
    );
    assert!(!conf.contains("autodetect"));
    assert!(conf.contains("MODULES=(btrfs)"));
}

#[test]
fn one_preset_per_kernel() {
    let steps = base_steps(
        &cfg("[boot]\nkernels = [\"linux\", \"linux-lts\"]\n"),
        &ctx("pw"),
    );
    let p = file(&steps, "/mnt/etc/mkinitcpio.d/linux.preset");
    assert!(p.contains("ALL_kver=/boot/vmlinuz-linux\n"), "{p}");
    assert!(p.contains("default_image=/boot/initramfs-linux.img"), "{p}");
    let p = file(&steps, "/mnt/etc/mkinitcpio.d/linux-lts.preset");
    assert!(p.contains("ALL_kver=/boot/vmlinuz-linux-lts\n"), "{p}");
    assert!(
        p.contains("default_image=/boot/initramfs-linux-lts.img"),
        "{p}"
    );
    assert!(
        argvs(&steps)
            .iter()
            .any(|a| a == &["arch-chroot", "/mnt", "mkinitcpio", "-P"])
    );
}

#[test]
fn locale_hostname_keymap_files_follow_config() {
    let steps = base_steps(
        &cfg("[system]\nhostname = \"zzhostzz\"\nlocale = \"de_DE.UTF-8\"\nkeymap = \"de\"\n"),
        &ctx("pw"),
    );
    assert_eq!(file(&steps, "/mnt/etc/hostname"), "zzhostzz\n");
    assert_eq!(file(&steps, "/mnt/etc/locale.conf"), "LANG=de_DE.UTF-8\n");
    assert_eq!(file(&steps, "/mnt/etc/vconsole.conf"), "KEYMAP=de\n");
    assert_eq!(
        file(&steps, "/mnt/etc/locale.gen"),
        "en_US.UTF-8 UTF-8\nde_DE.UTF-8 UTF-8\n"
    );
}

#[test]
fn locale_gen_has_no_duplicate_line_for_en_us() {
    let steps = base_steps(&cfg("[system]\nlocale = \"en_US.UTF-8\"\n"), &ctx("pw"));
    assert_eq!(file(&steps, "/mnt/etc/locale.gen"), "en_US.UTF-8 UTF-8\n");
}

#[test]
fn fstab_is_generated_after_pacstrap() {
    let a = argvs(&base_steps(&cfg(""), &ctx("pw")));
    let at = |pred: &dyn Fn(&Vec<String>) -> bool| a.iter().position(pred).unwrap();
    let pac = at(&|c| c[0] == "pacstrap");
    let fstab = at(&|c| c.iter().any(|x| x.contains("genfstab -U /mnt")));
    assert!(pac < fstab);
}

#[test]
fn admin_user_and_sudoers() {
    let steps = base_steps(&cfg(""), &ctx("pw"));
    assert!(argvs(&steps).iter().any(|a| a
        == &[
            "arch-chroot",
            "/mnt",
            "useradd",
            "-m",
            "-G",
            "wheel",
            "zzadminzz"
        ]));
    let sudoers = steps.iter().find_map(|s| match s {
        Step::WriteFile {
            path,
            contents,
            mode,
            ..
        } if path == "/mnt/etc/sudoers.d/10-wheel" => Some((contents.clone(), *mode)),
        _ => None,
    });
    assert_eq!(
        sudoers,
        Some(("%wheel ALL=(ALL:ALL) ALL\n".to_string(), 0o440))
    );
}

#[test]
fn chpasswd_gets_password_on_stdin_only() {
    let steps = base_steps(&cfg(""), &ctx("s3cret-pw"));
    let mut stdins = Vec::new();
    for s in &steps {
        if let Step::Run { cmd, .. } = s {
            assert!(
                cmd.argv.iter().all(|a| !a.contains("s3cret-pw")),
                "{:?}",
                cmd.argv
            );
            if let Some(sec) = &cmd.stdin {
                assert_eq!(&cmd.argv[..3], ["arch-chroot", "/mnt", "chpasswd"]);
                stdins.push(sec.expose().to_string());
            }
        }
        if let Step::WriteFile { contents, .. } = s {
            assert!(!contents.contains("s3cret-pw"));
        }
    }
    assert_eq!(stdins, ["root:s3cret-pw", "zzadminzz:s3cret-pw"]);
}

#[test]
fn password_with_colon_and_quotes_kept_verbatim() {
    let pw = "a:b'c\"d $e `f` \\g";
    let steps = base_steps(&cfg(""), &ctx(pw));
    let found = steps.iter().any(|s| {
        matches!(s, Step::Run { cmd, .. }
        if cmd.stdin.as_ref().is_some_and(|x| x.expose() == format!("zzadminzz:{pw}")))
    });
    assert!(found);
}

#[test]
fn empty_or_newline_password_rejected() {
    for pw in ["", "a\nb", "a\rb", "a\0b"] {
        assert!(Ctx::new("/dev/sda", PARTUUID, pw).is_err(), "{pw:?}");
    }
}

#[test]
fn bad_ctx_inputs_rejected() {
    assert!(Ctx::new("sda", PARTUUID, "pw").is_err());
    assert!(Ctx::new("/dev/sda", "not-a-uuid", "pw").is_err());
}

#[test]
fn no_config_value_is_passed_through_sh_c() {
    let c = cfg(
        "[system]\nhostname = \"zzhostzz\"\nlocale = \"de_DE.UTF-8\"\nkeymap = \"zzkeymap\"\ntimezone = \"Zz/Tzzz\"\n",
    );
    for a in argvs(&base_steps(&c, &ctx("pw"))) {
        let joined = a.join(" ");
        if joined.contains("sh -c") {
            for v in ["zzhostzz", "de_DE", "zzkeymap", "Zz/Tzzz", "zzadminzz"] {
                assert!(!joined.contains(v), "{v} leaked into {joined}");
            }
        }
    }
}

#[test]
fn timezone_and_locale_generation_run_in_the_chroot() {
    let a = argvs(&base_steps(
        &cfg("[system]\ntimezone = \"Europe/Berlin\"\n"),
        &ctx("pw"),
    ));
    assert!(a.iter().any(|c| c
        == &[
            "arch-chroot",
            "/mnt",
            "ln",
            "-sf",
            "/usr/share/zoneinfo/Europe/Berlin",
            "/etc/localtime"
        ]));
    assert!(
        a.iter()
            .any(|c| c == &["arch-chroot", "/mnt", "locale-gen"])
    );
}
