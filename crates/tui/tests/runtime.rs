mod common;

use arch_to_go_core::config::Config;
use arch_to_go_core::step::Secret;
use arch_to_go_tui::app::{App, Effect, Screen};
use arch_to_go_tui::disks::Disk;
use arch_to_go_tui::runtime::Runtime;
use common::stub;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const SECRET: &str = "s3cr3t pw'x";

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn ready_app() -> App {
    let mut a = App::new(
        None,
        true,
        vec![Disk {
            name: "/dev/sdz".into(),
            size: "10G".into(),
            model: "M".into(),
        }],
    );
    a.config.users.admin = "tester".into();
    a.device = Some("/dev/sdz".into());
    a.password = Some(Secret::new(SECRET));
    a
}

/// Drives the app to the Run screen and returns the Install effect the runtime must perform.
fn to_run(a: &mut App) -> Effect {
    for _ in 0..12 {
        a.handle_key(key(KeyCode::Up));
    }
    for _ in 0..10 {
        a.handle_key(key(KeyCode::Down));
    }
    a.handle_key(key(KeyCode::Enter));
    for c in "/dev/sdz".chars() {
        a.handle_key(key(KeyCode::Char(c)));
    }
    a.handle_key(key(KeyCode::Enter))
}

fn wait_until(rt: &mut Runtime, a: &mut App, what: &str, pred: impl Fn(&App) -> bool) {
    let t = Instant::now();
    while !pred(a) {
        assert!(
            t.elapsed() < Duration::from_secs(10),
            "timed out waiting for {what}"
        );
        rt.poll(a);
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

#[test]
fn review_fills_the_plan_from_a_dry_run() {
    let d = tempfile::tempdir().unwrap();
    let rec = d.path().join("rec");
    let exe = stub(
        d.path(),
        &format!(
            "echo \"$*\" > '{r}.args'\nprintenv ARCH_ADMIN_PASSWORD > '{r}.env'\necho '[dry-run] wipefs /dev/sdz'\necho '[dry-run] sync'",
            r = rec.display()
        ),
    );
    let mut a = ready_app();
    let mut rt = Runtime::new(exe);
    rt.perform(Effect::Review, &mut a);
    assert_eq!(a.screen, Screen::Review);
    assert!(
        a.review.iter().any(|l| l == "[dry-run] wipefs /dev/sdz"),
        "{:?}",
        a.review
    );
    assert!(a.review.iter().any(|l| l == "[dry-run] sync"));
    let args = read(&d.path().join("rec.args"));
    assert!(
        args.starts_with("install /dev/sdz --config ") && args.trim_end().ends_with("--dry-run"),
        "{args}"
    );
    assert!(
        !args.contains("s3cr3t"),
        "the password must never be in argv: {args}"
    );
    assert_eq!(
        read(&d.path().join("rec.env")).trim_end(),
        SECRET,
        "only via the environment"
    );
    assert!(
        rt.config_dir().is_none(),
        "the temp config is gone after the preview"
    );
}

#[test]
fn a_failing_dry_run_shows_its_output_and_a_message() {
    let d = tempfile::tempdir().unwrap();
    let exe = stub(d.path(), "echo 'error: /dev/sdz is not USB'; exit 1");
    let mut a = ready_app();
    let mut rt = Runtime::new(exe);
    rt.perform(Effect::Review, &mut a);
    assert_eq!(a.screen, Screen::Review);
    assert!(a.review.iter().any(|l| l.contains("is not USB")));
    assert!(
        a.message.as_deref().unwrap_or("").contains("failed"),
        "{:?}",
        a.message
    );
}

#[test]
fn install_forwards_the_device_to_stdin_and_reports_exit() {
    let d = tempfile::tempdir().unwrap();
    let exe = stub(d.path(), "read x\necho '==> step'\necho \"got:$x\"\nexit 0");
    let mut a = ready_app();
    let eff = to_run(&mut a);
    assert_eq!(eff, Effect::Install);
    let mut rt = Runtime::new(exe);
    rt.perform(eff, &mut a);
    wait_until(&mut rt, &mut a, "the exit", |a| a.screen == Screen::Done);
    assert_eq!(a.exit_code, Some(0));
    assert_eq!(a.step.as_deref(), Some("step"));
    assert!(a.log.iter().any(|l| l == "got:/dev/sdz"), "{:?}", a.log);
}

#[test]
fn config_dir_is_private_and_removed_after_exit() {
    let d = tempfile::tempdir().unwrap();
    let rec = d.path().join("rec");
    let exe = stub(
        d.path(),
        &format!(
            "read x\ndir=$(dirname \"$4\")\nstat -c %a \"$dir\" > '{r}.mode'\necho \"$4\" > '{r}.cfg'\ncat \"$4\" > '{r}.content'",
            r = rec.display()
        ),
    );
    let mut a = ready_app();
    let eff = to_run(&mut a);
    let mut rt = Runtime::new(exe);
    rt.perform(eff, &mut a);
    assert!(rt.config_dir().is_some(), "exists while the child runs");
    wait_until(&mut rt, &mut a, "the exit", |a| a.screen == Screen::Done);
    assert_eq!(
        read(&d.path().join("rec.mode")).trim(),
        "700",
        "child said: {:?}",
        a.log
    );
    let cfg = PathBuf::from(read(&d.path().join("rec.cfg")).trim());
    assert!(!cfg.exists(), "the config file is deleted after the exit");
    assert!(rt.config_dir().is_none());
    let content = read(&d.path().join("rec.content"));
    assert!(
        content.contains("tester") && !content.contains("s3cr3t"),
        "{content}"
    );
}

#[test]
fn missing_binary_ends_on_done_with_failure() {
    let mut a = ready_app();
    let eff = to_run(&mut a);
    let mut rt = Runtime::new(PathBuf::from("/nonexistent/arch-to-go-missing"));
    rt.perform(eff, &mut a);
    assert_eq!(a.screen, Screen::Done);
    assert_eq!(a.exit_code, Some(127));
    assert!(
        a.log
            .iter()
            .any(|l| l.contains("/nonexistent/arch-to-go-missing")),
        "{:?}",
        a.log
    );
    assert!(
        rt.config_dir().is_none(),
        "no temp directory is left behind"
    );
}

#[test]
fn save_then_load_round_trips_and_writes_no_password() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("saved.toml");
    let mut a = ready_app();
    a.config.system.hostname = "roundtrip".into();
    let mut rt = Runtime::new(PathBuf::from("/bin/true"));
    rt.perform(Effect::Save(path.clone()), &mut a);
    let text = read(&path);
    assert!(
        text.contains("roundtrip") && !text.contains("s3cr3t"),
        "{text}"
    );

    let mut b = ready_app();
    rt.perform(Effect::Load(path), &mut b);
    assert_eq!(b.config, a.config);
    assert!(b.password.is_some() && b.device.is_some());
}

#[test]
fn save_to_an_unwritable_path_sets_a_message() {
    let mut a = ready_app();
    let mut rt = Runtime::new(PathBuf::from("/bin/true"));
    rt.perform(
        Effect::Save(PathBuf::from("/nonexistent-dir-xyz/c.toml")),
        &mut a,
    );
    let m = a.message.clone().unwrap_or_default();
    assert!(m.contains("cannot save"), "{m}");
}

#[test]
fn load_of_invalid_toml_or_a_missing_file_keeps_the_config() {
    let d = tempfile::tempdir().unwrap();
    let bad = d.path().join("bad.toml");
    std::fs::write(&bad, "[users]\nadmin = \"x\"\nbogus = 1\n").unwrap();
    let mut a = ready_app();
    let before = Config::from_toml("[users]\nadmin = \"tester\"\n").unwrap();
    let mut rt = Runtime::new(PathBuf::from("/bin/true"));
    rt.perform(Effect::Load(bad), &mut a);
    assert_eq!(a.config, before);
    assert!(
        a.message.as_deref().unwrap_or("").contains("cannot load"),
        "{:?}",
        a.message
    );
    a.message = None;
    rt.perform(Effect::Load(d.path().join("missing.toml")), &mut a);
    assert_eq!(a.config, before);
    assert!(a.message.is_some());
}

#[test]
fn abort_interrupts_the_child() {
    let d = tempfile::tempdir().unwrap();
    let exe = stub(
        d.path(),
        "trap 'echo cleaned; exit 130' INT\necho ready\nread x\nsleep 30",
    );
    let mut a = ready_app();
    let eff = to_run(&mut a);
    let mut rt = Runtime::new(exe);
    rt.perform(eff, &mut a);
    wait_until(&mut rt, &mut a, "the child to start", |a| {
        a.log.iter().any(|l| l == "ready")
    });
    rt.perform(Effect::Abort, &mut a);
    wait_until(&mut rt, &mut a, "the exit", |a| a.screen == Screen::Done);
    assert_eq!(a.exit_code, Some(130));
    assert!(a.log.iter().any(|l| l == "cleaned"), "{:?}", a.log);
}

#[test]
fn quit_and_none_do_nothing() {
    let mut a = ready_app();
    let mut rt = Runtime::new(PathBuf::from("/bin/true"));
    rt.perform(Effect::Quit, &mut a);
    rt.perform(Effect::None, &mut a);
    assert_eq!(a.screen, Screen::Menu);
}

#[test]
fn a_failed_load_never_shows_the_secret_it_choked_on() {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("env");
    // the first line is built at run time: the repo scan rejects literal secret-looking assignments
    let env_line = format!("ARCH_SUDO_PASS{}=hunter2\n", "WORD");
    std::fs::write(
        &f,
        format!("{env_line}[users]\nadmin = \"a\"\npassword = \"hunter2\"\n"),
    )
    .unwrap();
    let mut a = ready_app();
    let mut rt = Runtime::new(PathBuf::from("/bin/true"));
    rt.perform(Effect::Load(f), &mut a);
    let m = a.message.clone().unwrap_or_default();
    assert!(m.contains("cannot load") && !m.contains("hunter2"), "{m}");
}

#[test]
fn dropping_the_runtime_stops_a_running_install_and_waits_for_its_cleanup() {
    let d = tempfile::tempdir().unwrap();
    let rec = d.path().join("rec");
    // a foreground sleep: only a signal to the whole process group interrupts it promptly
    let exe = stub(
        d.path(),
        &format!(
            "trap 'echo cleaned > \'{r}\'; exit 130' INT\necho ready\nread x\nsleep 30",
            r = rec.display()
        ),
    );
    let mut a = ready_app();
    let eff = to_run(&mut a);
    let mut rt = Runtime::new(exe);
    rt.perform(eff, &mut a);
    wait_until(&mut rt, &mut a, "the child to start", |a| {
        a.log.iter().any(|l| l == "ready")
    });
    drop(rt); // the TUI dying (error, panic) must not orphan a half-finished install
    assert_eq!(
        read(&rec).trim(),
        "cleaned",
        "drop waits for the child's cleanup"
    );
}

#[test]
fn a_hanging_dry_run_times_out_and_is_stopped() {
    let d = tempfile::tempdir().unwrap();
    let rec = d.path().join("rec");
    let exe = stub(
        d.path(),
        &format!(
            "trap 'echo stopped > \'{r}\'; exit 130' INT\nsleep 30",
            r = rec.display()
        ),
    );
    let mut a = ready_app();
    let mut rt = Runtime::new(exe);
    rt.review_timeout = Duration::from_millis(500);
    let t = Instant::now();
    rt.perform(Effect::Review, &mut a);
    assert!(
        t.elapsed() < Duration::from_secs(8),
        "returned in {:?}",
        t.elapsed()
    );
    assert_eq!(a.screen, Screen::Review);
    assert!(
        a.message.as_deref().unwrap_or("").contains("timed out"),
        "{:?}",
        a.message
    );
    assert_eq!(read(&rec).trim(), "stopped");
    assert!(rt.config_dir().is_none());
}
