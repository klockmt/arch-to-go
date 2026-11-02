use arch_to_go_core::config::{Config, Fs, Kernel};

fn parse(extra: &str) -> Result<Config, arch_to_go_core::config::ConfigError> {
    Config::from_toml(&format!("{extra}\n[users]\nadmin = \"tester\"\n"))
}

#[test]
fn minimal_config_parses_with_defaults() {
    let cfg = Config::from_toml(include_str!("fixtures/minimal.toml")).unwrap();
    assert_eq!(cfg.system.hostname, "archtogo");
    assert_eq!(cfg.disk.fs, Fs::Btrfs);
    assert_eq!(cfg.boot.kernels, vec![Kernel::Linux]);
    assert_eq!(
        cfg.disk.effective_subvolumes(),
        vec!["@", "@home", "@log", "@pkg"]
    );
    assert_eq!(cfg.users.admin, "tester");
}

#[test]
fn unknown_key_rejected() {
    assert!(parse("[disk]\nbogus = 1\n").is_err());
}

#[test]
fn password_key_rejected() {
    // a password must never live in the config file
    assert!(Config::from_toml("[users]\nadmin = \"a\"\npassword = \"x\"\n").is_err());
}

#[test]
fn missing_admin_rejected() {
    assert!(Config::from_toml("[system]\nhostname = \"x\"\n").is_err());
}

#[test]
fn empty_kernels_rejected() {
    assert!(parse("[boot]\nkernels = []\n").is_err());
}

#[test]
fn duplicate_kernel_rejected() {
    assert!(parse("[boot]\nkernels = [\"linux\", \"linux\"]\n").is_err());
}

#[test]
fn duplicate_package_rejected() {
    let e = parse("packages = [\"vim\", \"vim\"]\n")
        .unwrap_err()
        .to_string();
    assert!(e.contains("duplicate package"), "{e}");
}

#[test]
fn bad_package_name_rejected() {
    let e = parse("packages = [\"vim; rm\"]\n").unwrap_err().to_string();
    assert!(e.contains("package name"), "{e}");
}

#[test]
fn valid_top_level_lists_are_accepted() {
    let cfg = parse("packages = [\"vim\"]\nservices = [\"sshd.service\"]\n").unwrap();
    assert_eq!(cfg.packages, ["vim"]);
    assert_eq!(cfg.services, ["sshd.service"]);
}

#[test]
fn bad_hostname_rejected() {
    for h in ["a b", "-x", "x;rm", ""] {
        assert!(
            parse(&format!("[system]\nhostname = \"{h}\"\n")).is_err(),
            "{h:?}"
        );
    }
}

#[test]
fn bad_username_rejected() {
    for u in ["Root", "a:b", "", "a b"] {
        let toml = format!("[users]\nadmin = \"{u}\"\n");
        assert!(Config::from_toml(&toml).is_err(), "{u:?}");
    }
}

#[test]
fn bad_timezone_rejected() {
    for tz in ["../etc/passwd", "/etc", "A B", "a;b"] {
        assert!(
            parse(&format!("[system]\ntimezone = \"{tz}\"\n")).is_err(),
            "{tz:?}"
        );
    }
}

#[test]
fn bad_locale_and_keymap_rejected() {
    assert!(parse("[system]\nlocale = \"pt_BR; id\"\n").is_err());
    assert!(parse("[system]\nkeymap = \"br abnt2\"\n").is_err());
}

#[test]
fn btrfs_subvolumes_with_ext4_rejected() {
    assert!(parse("[disk]\nfs = \"ext4\"\nsubvolumes = [\"@\"]\n").is_err());
}

#[test]
fn ext4_has_no_default_subvolumes() {
    let cfg = parse("[disk]\nfs = \"ext4\"\n").unwrap();
    assert!(cfg.disk.effective_subvolumes().is_empty());
}

#[test]
fn btrfs_subvolumes_must_include_root() {
    assert!(parse("[disk]\nsubvolumes = [\"@home\"]\n").is_err());
}

#[test]
fn bad_esp_size_rejected() {
    assert!(parse("[disk]\nesp_size = \"1 G\"\n").is_err());
}

#[test]
fn kernels_parse_from_strings() {
    let cfg = parse("[boot]\nkernels = [\"linux\", \"linux-lts\"]\n").unwrap();
    assert_eq!(cfg.boot.kernels, vec![Kernel::Linux, Kernel::LinuxLts]);
}
