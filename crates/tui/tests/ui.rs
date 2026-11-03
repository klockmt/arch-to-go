use arch_to_go_core::step::Secret;
use arch_to_go_tui::app::{App, ChildEvent, Section};
use arch_to_go_tui::disks::Disk;
use arch_to_go_tui::ui::{MIN_HEIGHT, MIN_WIDTH, draw};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn app() -> App {
    let mut a = App::new(
        None,
        true,
        vec![Disk {
            name: "/dev/sdz".into(),
            size: "111,8G".into(),
            model: "TEST SSD".into(),
        }],
    );
    a.config.users.admin = "tester".into();
    a
}

fn ready() -> App {
    let mut a = app();
    a.device = Some("/dev/sdz".into());
    a.password = Some(Secret::new("pw"));
    a
}

fn menu_enter(a: &mut App, idx: usize) {
    for _ in 0..12 {
        a.handle_key(key(KeyCode::Up));
    }
    for _ in 0..idx {
        a.handle_key(key(KeyCode::Down));
    }
    a.handle_key(key(KeyCode::Enter));
}

fn typ(a: &mut App, s: &str) {
    for c in s.chars() {
        a.handle_key(key(KeyCode::Char(c)));
    }
}

/// The screen as text, one line per row.
fn render(a: &App, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| draw(a, f)).unwrap();
    let buf = t.backend().buffer();
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn screen(a: &App) -> String {
    render(a, 80, 24)
}

fn line_with<'a>(text: &'a str, needle: &str) -> &'a str {
    text.lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no line with {needle:?} in:\n{text}"))
}

#[test]
fn menu_lists_every_entry_and_flags_invalid_sections() {
    let t = screen(&app());
    for label in [
        "Disk", "Boot", "System", "User", "Profile", "Network", "Extras", "Save", "Load", "Review",
        "Install", "Quit",
    ] {
        assert!(t.contains(label), "{label} missing:\n{t}");
    }
    assert!(
        line_with(&t, "Disk").contains('!'),
        "Disk is invalid (no device)"
    );
    assert!(
        line_with(&t, "User").contains('!'),
        "User is invalid (no password)"
    );
    assert!(!line_with(&t, "Boot").contains('!'));
}

#[test]
fn install_row_shows_disabled_until_ready() {
    assert!(line_with(&screen(&app()), "Install").contains("not ready"));
    let t = screen(&ready());
    let l = line_with(&t, "Install");
    assert!(l.contains("ready") && !l.contains("not ready"), "{l}");
    let mut user = ready();
    user.is_root = false;
    assert!(line_with(&screen(&user), "Install").contains("needs root"));
}

#[test]
fn section_screen_shows_rows_and_a_cursor() {
    let mut a = app();
    menu_enter(&mut a, 0);
    let t = screen(&a);
    for s in ["Device", "Filesystem", "ESP size", "btrfs"] {
        assert!(t.contains(s), "{s}:\n{t}");
    }
    assert!(
        line_with(&t, "Device").contains('>'),
        "cursor on the first row"
    );
}

#[test]
fn user_screen_never_shows_the_password() {
    let secret = "p'a:s$s wörd";
    let mut a = app();
    menu_enter(&mut a, 3);
    a.handle_key(key(KeyCode::Down));
    for _ in 0..2 {
        a.handle_key(key(KeyCode::Enter));
        typ(&mut a, secret);
        a.handle_key(key(KeyCode::Enter));
        a.handle_key(key(KeyCode::Down));
    }
    let t = screen(&a);
    assert!(t.contains(&"*".repeat(secret.chars().count())), "{t}");
    for leak in ["wörd", "p'a", "s$s"] {
        assert!(!t.contains(leak), "{leak} leaked:\n{t}");
    }
}

#[test]
fn editing_a_masked_field_shows_only_stars() {
    let mut a = app();
    menu_enter(&mut a, 3);
    a.handle_key(key(KeyCode::Down));
    a.handle_key(key(KeyCode::Enter));
    typ(&mut a, "hunter2");
    let t = screen(&a);
    assert!(!t.contains("hunter2"), "{t}");
    assert!(t.contains("*******"), "{t}");
}

#[test]
fn review_shows_plan_lines() {
    let mut a = ready();
    a.set_review(vec![
        "[dry-run] wipefs --all /dev/sdz".into(),
        "[dry-run] sync".into(),
    ]);
    let t = screen(&a);
    assert!(
        t.contains("wipefs --all /dev/sdz") && t.contains("[dry-run] sync"),
        "{t}"
    );
}

#[test]
fn confirm_shows_the_device_and_typed_text() {
    let mut a = ready();
    menu_enter(&mut a, 10);
    typ(&mut a, "/dev/sd");
    let t = screen(&a);
    assert!(t.contains("ERASE") && t.contains("/dev/sdz"), "{t}");
    assert!(t.contains("/dev/sd_"), "typed text with a cursor:\n{t}");
}

#[test]
fn run_shows_current_step_and_last_log_lines() {
    let mut a = ready();
    menu_enter(&mut a, 10);
    typ(&mut a, "/dev/sdz");
    a.handle_key(key(KeyCode::Enter));
    a.on_event(ChildEvent::Line("==> install base system".into()));
    for i in 0..100 {
        a.on_event(ChildEvent::Line(format!("downloading {i}")));
    }
    let t = screen(&a);
    assert!(t.contains("install base system"), "{t}");
    assert!(t.contains("downloading 99"), "last line is visible:\n{t}");
    assert!(!t.contains("downloading 3 "), "old lines scroll away:\n{t}");
    assert!(t.contains("Ctrl-C"), "{t}");
}

#[test]
fn done_shows_exit_code_and_last_lines() {
    let mut a = ready();
    menu_enter(&mut a, 10);
    typ(&mut a, "/dev/sdz");
    a.handle_key(key(KeyCode::Enter));
    a.on_event(ChildEvent::Line("error: something broke".into()));
    a.on_event(ChildEvent::Exited(3));
    let t = screen(&a);
    assert!(t.contains("failed") && t.contains('3'), "{t}");
    assert!(t.contains("something broke"), "{t}");
    let mut ok = ready();
    menu_enter(&mut ok, 10);
    typ(&mut ok, "/dev/sdz");
    ok.handle_key(key(KeyCode::Enter));
    ok.on_event(ChildEvent::Exited(0));
    assert!(screen(&ok).contains("finished"), "{}", screen(&ok));
}

#[test]
fn messages_and_prompts_are_drawn() {
    let mut a = app();
    menu_enter(&mut a, 10); // Install while not ready: sets a message
    assert!(screen(&a).contains("not ready"), "{}", screen(&a));
    let mut b = ready();
    menu_enter(&mut b, 7); // Save prompt
    assert!(screen(&b).contains("config.toml"), "{}", screen(&b));
}

#[test]
fn too_small_terminals_do_not_panic() {
    let a = app();
    for (w, h) in [(0, 0), (1, 1), (79, 23), (80, 23), (79, 24), (200, 3)] {
        let t = render(&a, w, h);
        if w > 30 && h > 0 {
            assert!(t.contains("terminal too small"), "{w}x{h}:\n{t}");
        }
    }
    assert!(render(&a, MIN_WIDTH, MIN_HEIGHT).contains("Disk"));
    assert_eq!((MIN_WIDTH, MIN_HEIGHT), (80, 24));
}

#[test]
fn a_huge_terminal_and_long_lines_do_not_panic() {
    let mut a = ready();
    menu_enter(&mut a, 10);
    typ(&mut a, "/dev/sdz");
    a.handle_key(key(KeyCode::Enter));
    a.on_event(ChildEvent::Line("x".repeat(5000)));
    a.on_event(ChildEvent::Line("héllo wörld ✓ ".repeat(300)));
    render(&a, 80, 24);
    render(&a, 300, 100);
    let mut s = app();
    menu_enter(&mut s, 6);
    s.handle_key(key(KeyCode::Enter));
    typ(&mut s, &"vim ".repeat(500));
    render(&s, 80, 24);
    assert_eq!(
        s.screen,
        arch_to_go_tui::app::Screen::Section(Section::Extras)
    );
}

#[test]
fn confirm_shows_which_disk_will_be_erased() {
    // the path alone does not tell a backup stick from the target: show size and model too
    let mut a = ready();
    menu_enter(&mut a, 10);
    let t = screen(&a);
    assert!(t.contains("111,8G") && t.contains("TEST SSD"), "{t}");
}

#[test]
fn menu_and_disk_screen_show_size_and_model() {
    let a = ready();
    let t = screen(&a);
    assert!(line_with(&t, "Disk").contains("TEST SSD"), "{t}");
    let mut b = ready();
    menu_enter(&mut b, 0);
    let t = screen(&b);
    assert!(line_with(&t, "Device").contains("111,8G"), "{t}");
}
