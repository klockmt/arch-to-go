use arch_to_go_core::config::Config;
use arch_to_go_core::step::Secret;
use arch_to_go_tui::app::{App, ChildEvent, Effect, Screen};
use arch_to_go_tui::disks::Disk;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn ctrl_c() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
}

fn ready(is_root: bool) -> App {
    let mut a = App::new(
        None,
        is_root,
        vec![Disk {
            name: "/dev/sdz".into(),
            size: "10G".into(),
            model: "M".into(),
        }],
    );
    a.config.users.admin = "tester".into();
    a.device = Some("/dev/sdz".into());
    a.password = Some(Secret::new("pw"));
    a
}

/// Presses Enter on menu entry `idx` (Save 7, Load 8, Review 9, Install 10).
fn menu_enter(a: &mut App, idx: usize) -> Effect {
    for _ in 0..12 {
        a.handle_key(key(KeyCode::Up));
    }
    for _ in 0..idx {
        a.handle_key(key(KeyCode::Down));
    }
    a.handle_key(key(KeyCode::Enter))
}

fn typ(a: &mut App, s: &str) -> Effect {
    let mut last = Effect::None;
    for c in s.chars() {
        last = a.handle_key(key(KeyCode::Char(c)));
    }
    last
}

#[test]
fn review_with_invalid_config_stays_on_menu_with_message() {
    let mut a = ready(true);
    a.config.system.hostname = "bad host".into();
    assert_eq!(menu_enter(&mut a, 9), Effect::None);
    assert_eq!(a.screen, Screen::Menu);
    assert!(a.message.is_some());
}

#[test]
fn review_needs_a_device() {
    let mut a = ready(true);
    a.device = None;
    assert_eq!(menu_enter(&mut a, 9), Effect::None);
    assert!(a.message.is_some());
}

#[test]
fn set_review_shows_the_plan_and_scrolls() {
    let mut a = ready(true);
    assert_eq!(menu_enter(&mut a, 9), Effect::Review);
    a.set_review((0..30).map(|i| format!("line {i}")).collect());
    assert_eq!(a.screen, Screen::Review);
    assert_eq!(a.review_scroll, 0);
    a.handle_key(key(KeyCode::Down));
    a.handle_key(key(KeyCode::PageDown));
    assert_eq!(a.review_scroll, 11);
    a.handle_key(key(KeyCode::PageUp));
    a.handle_key(key(KeyCode::Up));
    assert_eq!(a.review_scroll, 0);
    a.handle_key(key(KeyCode::Up));
    assert_eq!(a.review_scroll, 0, "does not go below the top");
    a.handle_key(key(KeyCode::Esc));
    assert_eq!(a.screen, Screen::Menu);
}

#[test]
fn install_without_root_shows_the_sudo_command() {
    let mut a = ready(false);
    assert_eq!(menu_enter(&mut a, 10), Effect::None);
    assert_eq!(a.screen, Screen::Menu);
    let m = a.message.clone().unwrap();
    assert!(
        m.contains("sudo arch-to-go install /dev/sdz --config"),
        "{m}"
    );
}

#[test]
fn install_when_not_ready_says_so_even_as_root() {
    let mut a = ready(true);
    a.password = None;
    assert_eq!(menu_enter(&mut a, 10), Effect::None);
    assert_eq!(a.screen, Screen::Menu);
    assert!(a.message.is_some());
}

#[test]
fn confirm_requires_the_exact_path() {
    for wrong in ["/dev/sdz ", "sdz", "/dev/SDZ", ""] {
        let mut a = ready(true);
        menu_enter(&mut a, 10);
        assert_eq!(a.screen, Screen::Confirm);
        typ(&mut a, wrong);
        assert_eq!(a.handle_key(key(KeyCode::Enter)), Effect::None, "{wrong:?}");
        assert_eq!(a.screen, Screen::Confirm);
        assert!(a.message.is_some());
    }
    let mut a = ready(true);
    menu_enter(&mut a, 10);
    typ(&mut a, "/dev/sdz");
    assert_eq!(a.confirm_text(), "/dev/sdz");
    assert_eq!(a.handle_key(key(KeyCode::Enter)), Effect::Install);
    assert_eq!(a.screen, Screen::Run);
}

#[test]
fn confirm_backspace_edits_and_esc_goes_back() {
    let mut a = ready(true);
    menu_enter(&mut a, 10);
    typ(&mut a, "/dev/sdzz");
    a.handle_key(key(KeyCode::Backspace));
    assert_eq!(a.confirm_text(), "/dev/sdz");
    a.handle_key(key(KeyCode::Esc));
    assert_eq!(a.screen, Screen::Menu);
    menu_enter(&mut a, 10);
    assert_eq!(a.confirm_text(), "", "a fresh confirmation starts empty");
}

fn running() -> App {
    let mut a = ready(true);
    menu_enter(&mut a, 10);
    typ(&mut a, "/dev/sdz");
    a.handle_key(key(KeyCode::Enter));
    a
}

#[test]
fn step_lines_update_progress_and_log() {
    let mut a = running();
    a.on_event(ChildEvent::Line("==> partition".into()));
    a.on_event(ChildEvent::Line("sgdisk: ok".into()));
    assert_eq!(a.step.as_deref(), Some("partition"));
    assert_eq!(a.log, ["==> partition", "sgdisk: ok"]);
}

#[test]
fn exited_moves_to_done_with_code() {
    let mut a = running();
    a.on_event(ChildEvent::Exited(3));
    assert_eq!(a.screen, Screen::Done);
    assert_eq!(a.exit_code, Some(3));
}

#[test]
fn ctrl_c_in_run_aborts_once() {
    let mut a = running();
    assert_eq!(a.handle_key(ctrl_c()), Effect::Abort);
    assert!(a.message.as_deref().unwrap_or("").contains("aborting"));
    assert_eq!(a.handle_key(ctrl_c()), Effect::None);
    assert_eq!(a.handle_key(key(KeyCode::Esc)), Effect::None);
    assert_eq!(
        a.screen,
        Screen::Run,
        "only the child's exit leaves the Run screen"
    );
}

#[test]
fn done_enter_returns_to_menu_and_q_quits() {
    let mut a = running();
    a.on_event(ChildEvent::Exited(0));
    assert_eq!(a.handle_key(key(KeyCode::Char('q'))), Effect::Quit);
    assert_eq!(a.handle_key(key(KeyCode::Enter)), Effect::None);
    assert_eq!(a.screen, Screen::Menu);
}

#[test]
fn a_second_install_starts_with_a_clean_log() {
    let mut a = running();
    a.on_event(ChildEvent::Line("old".into()));
    a.on_event(ChildEvent::Exited(1));
    a.handle_key(key(KeyCode::Enter));
    menu_enter(&mut a, 10);
    typ(&mut a, "/dev/sdz");
    a.handle_key(key(KeyCode::Enter));
    assert!(a.log.is_empty());
    assert_eq!(a.exit_code, None);
    assert_eq!(a.step, None);
}

#[test]
fn loading_keeps_device_and_password() {
    let mut a = ready(true);
    let cfg = Config::from_toml("[users]\nadmin = \"loaded\"\n[system]\nhostname = \"fromfile\"\n")
        .unwrap();
    a.apply_loaded(cfg);
    assert_eq!(a.config.system.hostname, "fromfile");
    assert_eq!(a.device.as_deref(), Some("/dev/sdz"));
    assert!(a.password.is_some());
}

#[test]
fn save_prompt_returns_the_typed_path() {
    let mut a = ready(true);
    assert_eq!(menu_enter(&mut a, 7), Effect::None, "opens the prompt");
    assert_eq!(a.prompt_text(), Some("config.toml"));
    assert_eq!(
        a.handle_key(key(KeyCode::Enter)),
        Effect::Save(PathBuf::from("config.toml"))
    );
    assert_eq!(a.prompt_text(), None);
    // a new path: erase the default, type another
    menu_enter(&mut a, 7);
    for _ in 0..11 {
        a.handle_key(key(KeyCode::Backspace));
    }
    typ(&mut a, "my.toml");
    assert_eq!(
        a.handle_key(key(KeyCode::Enter)),
        Effect::Save(PathBuf::from("my.toml"))
    );
}

#[test]
fn load_prompt_and_esc_cancels() {
    let mut a = ready(true);
    menu_enter(&mut a, 8);
    assert_eq!(a.handle_key(key(KeyCode::Esc)), Effect::None);
    assert_eq!(a.prompt_text(), None);
    assert_eq!(a.screen, Screen::Menu);
    menu_enter(&mut a, 8);
    assert_eq!(
        a.handle_key(key(KeyCode::Enter)),
        Effect::Load(PathBuf::from("config.toml"))
    );
}

#[test]
fn the_sudo_message_uses_the_saved_path() {
    let mut a = ready(false);
    menu_enter(&mut a, 7);
    a.handle_key(key(KeyCode::Enter)); // save config.toml
    menu_enter(&mut a, 10);
    assert!(a.message.clone().unwrap().contains("--config config.toml"));
}
