use arch_to_go_core::step::Secret;
use arch_to_go_tui::app::{App, Effect, Screen, Section};
use arch_to_go_tui::disks::Disk;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn disk() -> Disk {
    Disk {
        name: "/dev/sdz".into(),
        size: "111,8G".into(),
        model: "TEST SSD".into(),
    }
}

fn new_app() -> App {
    App::new(None, true, vec![disk()])
}

/// An app that can install: valid admin, a device and a password.
fn ready_app() -> App {
    let mut a = new_app();
    a.config.users.admin = "tester".into();
    a.device = Some("/dev/sdz".into());
    a.password = Some(Secret::new("pw"));
    a
}

fn press(a: &mut App, codes: &[KeyCode]) -> Effect {
    let mut last = Effect::None;
    for c in codes {
        last = a.handle_key(key(*c));
    }
    last
}

#[test]
fn starts_on_the_menu_with_install_disabled() {
    let a = new_app();
    assert_eq!(a.screen, Screen::Menu);
    assert!(!a.can_install());
}

#[test]
fn menu_has_the_twelve_entries_in_order() {
    let labels: Vec<String> = new_app()
        .menu_items()
        .into_iter()
        .map(|m| m.label)
        .collect();
    assert_eq!(
        labels,
        [
            "Disk", "Boot", "System", "User", "Profile", "Network", "Extras", "Save", "Load",
            "Review", "Install", "Quit"
        ]
    );
}

#[test]
fn down_up_and_enter_open_a_section() {
    let mut a = new_app();
    press(
        &mut a,
        &[KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Enter],
    );
    assert_eq!(a.screen, Screen::Section(Section::User));
    press(&mut a, &[KeyCode::Esc]);
    assert_eq!(a.screen, Screen::Menu);
    press(&mut a, &[KeyCode::Up, KeyCode::Enter]);
    assert_eq!(a.screen, Screen::Section(Section::System));
}

#[test]
fn install_needs_valid_config_disk_and_password() {
    let mut a = new_app();
    assert!(!a.can_install());
    a.config.users.admin = "tester".into();
    assert!(!a.can_install(), "no device yet");
    a.device = Some("/dev/sdz".into());
    assert!(!a.can_install(), "no password yet");
    a.password = Some(Secret::new("pw"));
    assert!(a.can_install());
    a.config.system.hostname = "bad host".into();
    assert!(!a.can_install(), "invalid config");
}

#[test]
fn no_usb_disk_is_flagged() {
    let mut a = App::new(None, true, vec![]);
    a.config.users.admin = "tester".into();
    a.password = Some(Secret::new("pw"));
    let disk_item = &a.menu_items()[0];
    assert!(
        disk_item.value.contains("no USB disk"),
        "{}",
        disk_item.value
    );
    assert!(disk_item.invalid);
    assert!(!a.can_install());
}

#[test]
fn invalid_section_is_flagged_on_its_own() {
    let mut a = ready_app();
    a.config.system.hostname = "bad host".into();
    let flagged: Vec<String> = a
        .menu_items()
        .into_iter()
        .filter(|m| m.invalid)
        .map(|m| m.label)
        .collect();
    assert_eq!(flagged, ["System"]);
}

#[test]
fn new_app_flags_user_and_disk() {
    let flagged: Vec<String> = new_app()
        .menu_items()
        .into_iter()
        .filter(|m| m.invalid)
        .map(|m| m.label)
        .collect();
    assert_eq!(flagged, ["Disk", "User"]);
}

#[test]
fn enter_on_install_when_disabled_does_nothing() {
    let mut a = new_app();
    let at = 10; // Install
    for _ in 0..at {
        a.handle_key(key(KeyCode::Down));
    }
    let eff = a.handle_key(key(KeyCode::Enter));
    assert_eq!(eff, Effect::None);
    assert_eq!(a.screen, Screen::Menu);
    assert!(a.message.is_some());
}

#[test]
fn enter_on_review_returns_review_effect() {
    let mut a = ready_app();
    for _ in 0..9 {
        a.handle_key(key(KeyCode::Down));
    }
    assert_eq!(a.handle_key(key(KeyCode::Enter)), Effect::Review);
}

#[test]
fn q_and_ctrl_c_quit_from_the_menu() {
    assert_eq!(new_app().handle_key(key(KeyCode::Char('q'))), Effect::Quit);
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(new_app().handle_key(ctrl_c), Effect::Quit);
}
