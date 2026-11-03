use arch_to_go_core::config::{Fs, Kernel, NetworkBackend, Profile};
use arch_to_go_tui::app::{App, Screen, Section};
use arch_to_go_tui::disks::Disk;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn disk(n: &str) -> Disk {
    Disk {
        name: n.into(),
        size: "10G".into(),
        model: "M".into(),
    }
}

fn app() -> App {
    let mut a = App::new(None, true, vec![disk("/dev/sdy"), disk("/dev/sdz")]);
    a.config.users.admin = "tester".into();
    a
}

/// Opens the section at menu position `idx` (Disk 0, Boot 1, System 2, User 3, Profile 4,
/// Network 5, Extras 6).
fn open(a: &mut App, idx: usize) {
    for _ in 0..12 {
        a.handle_key(key(KeyCode::Up)); // the menu remembers its cursor: start from the top
    }
    for _ in 0..idx {
        a.handle_key(key(KeyCode::Down));
    }
    a.handle_key(key(KeyCode::Enter));
}

fn down(a: &mut App, n: usize) {
    for _ in 0..n {
        a.handle_key(key(KeyCode::Down));
    }
}

fn typ(a: &mut App, s: &str) {
    for c in s.chars() {
        a.handle_key(key(KeyCode::Char(c)));
    }
}

fn enter(a: &mut App) {
    a.handle_key(key(KeyCode::Enter));
}

fn esc(a: &mut App) {
    a.handle_key(key(KeyCode::Esc));
}

#[test]
fn typing_edits_the_hostname_and_validation_shows_in_the_menu() {
    let mut a = app();
    open(&mut a, 2);
    enter(&mut a); // start editing the hostname
    typ(&mut a, " x");
    assert_eq!(a.config.system.hostname, "archtogo x");
    enter(&mut a); // commit
    esc(&mut a);
    assert_eq!(a.screen, Screen::Menu);
    let flagged: Vec<String> = a
        .menu_items()
        .into_iter()
        .filter(|m| m.invalid && m.label == "System")
        .map(|m| m.label)
        .collect();
    assert_eq!(flagged, ["System"]);
}

#[test]
fn esc_cancels_a_text_edit_and_restores_the_value() {
    let mut a = app();
    open(&mut a, 2);
    enter(&mut a);
    typ(&mut a, "zzz");
    esc(&mut a);
    assert_eq!(a.config.system.hostname, "archtogo");
    assert_eq!(
        a.screen,
        Screen::Section(Section::System),
        "Esc only cancels the edit"
    );
}

#[test]
fn unicode_text_edits_do_not_panic() {
    let mut a = app();
    open(&mut a, 3); // User: admin is the first row
    enter(&mut a);
    typ(&mut a, "héllo wörld ✓");
    assert_eq!(a.config.users.admin, "testerhéllo wörld ✓");
    for _ in 0..4 {
        a.handle_key(key(KeyCode::Backspace));
    }
    assert_eq!(a.config.users.admin, "testerhéllo wör");
}

#[test]
fn last_kernel_cannot_be_unselected() {
    let mut a = app();
    open(&mut a, 1);
    a.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(a.config.boot.kernels, [Kernel::Linux]);
    assert!(a.message.is_some());
}

#[test]
fn kernel_multi_select_updates_config() {
    let mut a = app();
    open(&mut a, 1);
    down(&mut a, 1);
    a.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(a.config.boot.kernels, [Kernel::Linux, Kernel::LinuxLts]);
    a.handle_key(key(KeyCode::Up));
    a.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(a.config.boot.kernels, [Kernel::LinuxLts]);
}

#[test]
fn picking_a_disk_sets_the_device() {
    let mut a = app();
    open(&mut a, 0);
    enter(&mut a);
    assert_eq!(a.device.as_deref(), Some("/dev/sdy"));
    enter(&mut a);
    assert_eq!(a.device.as_deref(), Some("/dev/sdz"));
    enter(&mut a);
    assert_eq!(a.device.as_deref(), Some("/dev/sdy"), "wraps around");
}

#[test]
fn fs_cycles_and_subvolumes_stay_consistent() {
    let mut a = app();
    a.config.disk.subvolumes = Some(vec!["@".into(), "@home".into()]);
    open(&mut a, 0);
    down(&mut a, 1);
    a.handle_key(key(KeyCode::Right));
    assert_eq!(a.config.disk.fs, Fs::Ext4);
    assert_eq!(
        a.config.disk.subvolumes, None,
        "subvolumes only apply to btrfs"
    );
    a.handle_key(key(KeyCode::Right));
    assert_eq!(a.config.disk.fs, Fs::Xfs);
    a.handle_key(key(KeyCode::Right));
    assert_eq!(a.config.disk.fs, Fs::Btrfs);
    a.handle_key(key(KeyCode::Left));
    assert_eq!(a.config.disk.fs, Fs::Xfs);
}

#[test]
fn password_must_match_and_be_non_empty() {
    let mut a = app();
    open(&mut a, 3);
    down(&mut a, 1);
    enter(&mut a);
    typ(&mut a, "secret");
    enter(&mut a);
    assert!(a.password.is_none(), "no repeat yet");
    down(&mut a, 1);
    enter(&mut a);
    typ(&mut a, "other");
    enter(&mut a);
    assert!(a.password.is_none());
    assert!(a.message.is_some());
    enter(&mut a);
    for _ in 0..5 {
        a.handle_key(key(KeyCode::Backspace));
    }
    typ(&mut a, "secret");
    enter(&mut a);
    assert_eq!(
        a.password
            .as_ref()
            .map(|p| p.expose().to_string())
            .as_deref(),
        Some("secret")
    );
}

#[test]
fn password_display_never_contains_the_secret() {
    let secret = "p'a:s$s wörd";
    let mut a = app();
    open(&mut a, 3);
    down(&mut a, 1);
    for _ in 0..2 {
        enter(&mut a);
        typ(&mut a, secret);
        enter(&mut a);
        down(&mut a, 1);
    }
    assert!(a.password.is_some());
    let shown = a.password_display();
    assert_eq!(shown.chars().count(), secret.chars().count());
    assert!(shown.chars().all(|c| c == '*'), "{shown}");
    let everything = format!(
        "{:?}{:?}",
        a.rows(Section::User)
            .iter()
            .map(|r| r.value.clone())
            .collect::<Vec<_>>(),
        a.menu_items()
    );
    assert!(
        !everything.contains("wörd") && !everything.contains("p'a"),
        "{everything}"
    );
}

#[test]
fn toggles_flip_config_flags() {
    let mut a = app();
    open(&mut a, 0);
    down(&mut a, 3);
    enter(&mut a);
    assert!(a.config.disk.nodatacow);
    down(&mut a, 1);
    a.handle_key(key(KeyCode::Char(' ')));
    assert!(a.config.disk.zram);
    esc(&mut a);

    open(&mut a, 4); // Profile
    down(&mut a, 1);
    enter(&mut a);
    assert!(a.config.profile.audio);
    esc(&mut a);

    open(&mut a, 6); // Extras: packages, services, paru, firmware, gpu
    down(&mut a, 2);
    enter(&mut a);
    assert!(a.config.aur.paru);
    down(&mut a, 1);
    enter(&mut a);
    assert!(!a.config.portable.firmware);
    down(&mut a, 1);
    enter(&mut a);
    assert!(!a.config.portable.gpu_drivers);
}

#[test]
fn profile_and_network_cycle() {
    let mut a = app();
    open(&mut a, 4);
    a.handle_key(key(KeyCode::Right));
    assert_eq!(a.config.profile.name, Profile::Hyprland);
    a.handle_key(key(KeyCode::Left));
    a.handle_key(key(KeyCode::Left));
    assert_eq!(a.config.profile.name, Profile::Kde, "wraps backwards");
    esc(&mut a);
    open(&mut a, 5);
    a.handle_key(key(KeyCode::Right));
    assert_eq!(a.config.network.backend, NetworkBackend::NetworkManager);
}

#[test]
fn extras_split_packages_on_whitespace() {
    let mut a = app();
    open(&mut a, 6);
    enter(&mut a);
    typ(&mut a, "vim  htop git");
    enter(&mut a);
    assert_eq!(a.config.packages, ["vim", "htop", "git"]);
    down(&mut a, 1);
    enter(&mut a);
    typ(&mut a, "sshd.service");
    enter(&mut a);
    assert_eq!(a.config.services, ["sshd.service"]);
}
