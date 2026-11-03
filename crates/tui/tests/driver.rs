use arch_to_go_tui::app::ChildEvent;
mod common;

use arch_to_go_tui::driver::{Child, install_args};
use common::stub;
use std::path::Path;
use std::time::Duration;

fn recv(c: &Child) -> ChildEvent {
    c.rx.recv_timeout(Duration::from_secs(10))
        .expect("event within 10 s")
}

/// Everything up to and including the exit.
fn until_exit(c: &Child) -> Vec<ChildEvent> {
    let mut out = Vec::new();
    loop {
        let ev = recv(c);
        let done = matches!(ev, ChildEvent::Exited(_));
        out.push(ev);
        if done {
            return out;
        }
    }
}

fn line(s: &str) -> ChildEvent {
    ChildEvent::Line(s.into())
}

#[test]
fn lines_and_exit_code_arrive_in_order() {
    let d = tempfile::tempdir().unwrap();
    let exe = stub(d.path(), "echo '==> one'; echo plain; exit 0");
    let c = Child::spawn(&exe, &[], &[]).unwrap();
    assert_eq!(
        until_exit(&c),
        [line("==> one"), line("plain"), ChildEvent::Exited(0)]
    );
}

#[test]
fn nonzero_exit_is_reported() {
    let d = tempfile::tempdir().unwrap();
    let exe = stub(d.path(), "exit 3");
    let c = Child::spawn(&exe, &[], &[]).unwrap();
    assert_eq!(until_exit(&c), [ChildEvent::Exited(3)]);
}

#[test]
fn stderr_lines_arrive_too() {
    let d = tempfile::tempdir().unwrap();
    let exe = stub(d.path(), "echo oops 1>&2");
    let c = Child::spawn(&exe, &[], &[]).unwrap();
    assert_eq!(until_exit(&c), [line("oops"), ChildEvent::Exited(0)]);
}

#[test]
fn partial_last_line_is_flushed() {
    let d = tempfile::tempdir().unwrap();
    let exe = stub(d.path(), "printf 'Type the path: '");
    let c = Child::spawn(&exe, &[], &[]).unwrap();
    assert_eq!(
        until_exit(&c),
        [line("Type the path: "), ChildEvent::Exited(0)]
    );
}

#[test]
fn stdin_line_reaches_the_child() {
    let d = tempfile::tempdir().unwrap();
    let exe = stub(d.path(), "read x; echo \"got:$x\"");
    let mut c = Child::spawn(&exe, &[], &[]).unwrap();
    c.send_line("/dev/sdz").unwrap();
    assert_eq!(
        until_exit(&c),
        [line("got:/dev/sdz"), ChildEvent::Exited(0)]
    );
}

#[test]
fn args_and_env_are_passed() {
    let d = tempfile::tempdir().unwrap();
    let exe = stub(d.path(), "echo \"v=$ARCH_TEST_PROBE a=$1\"");
    let env = [("ARCH_TEST_PROBE".to_string(), "fake-value".to_string())];
    let c = Child::spawn(&exe, &["one".to_string()], &env).unwrap();
    assert_eq!(
        until_exit(&c),
        [line("v=fake-value a=one"), ChildEvent::Exited(0)]
    );
}

#[test]
fn interrupt_stops_a_running_child_and_lets_it_clean_up() {
    let d = tempfile::tempdir().unwrap();
    let exe = stub(
        d.path(),
        "trap 'echo cleaned; exit 130' INT\necho ready\nsleep 30",
    );
    let c = Child::spawn(&exe, &[], &[]).unwrap();
    assert_eq!(recv(&c), line("ready"));
    c.interrupt();
    assert_eq!(until_exit(&c), [line("cleaned"), ChildEvent::Exited(130)]);
}

#[test]
fn a_signal_death_is_128_plus_the_signal() {
    let d = tempfile::tempdir().unwrap();
    let exe = stub(d.path(), "kill -TERM $$");
    let c = Child::spawn(&exe, &[], &[]).unwrap();
    assert_eq!(until_exit(&c), [ChildEvent::Exited(143)]);
}

#[test]
fn missing_binary_is_an_error_naming_the_path() {
    let err = Child::spawn(Path::new("/nonexistent/arch-to-go-stub"), &[], &[])
        .err()
        .expect("spawn must fail");
    assert!(err.contains("/nonexistent/arch-to-go-stub"), "{err}");
}

#[test]
fn invalid_utf8_does_not_stall_the_reader() {
    let d = tempfile::tempdir().unwrap();
    let exe = stub(d.path(), "printf 'a\\377b\\n'");
    let c = Child::spawn(&exe, &[], &[]).unwrap();
    let ev = until_exit(&c);
    assert_eq!(ev.len(), 2, "{ev:?}");
    assert!(matches!(&ev[0], ChildEvent::Line(l) if l.starts_with('a') && l.ends_with('b')));
}

#[test]
fn a_lot_of_output_does_not_deadlock() {
    let d = tempfile::tempdir().unwrap();
    let exe = stub(d.path(), "seq 1 50000");
    let c = Child::spawn(&exe, &[], &[]).unwrap();
    let ev = until_exit(&c);
    assert_eq!(ev.len(), 50001);
    assert_eq!(ev[49999], line("50000"));
}

#[test]
fn install_args_are_exact() {
    let cfg = Path::new("/tmp/x/config.toml");
    assert_eq!(
        install_args("/dev/sdz", cfg, false),
        ["install", "/dev/sdz", "--config", "/tmp/x/config.toml"]
    );
    assert_eq!(
        install_args("/dev/sdz", cfg, true),
        [
            "install",
            "/dev/sdz",
            "--config",
            "/tmp/x/config.toml",
            "--dry-run"
        ]
    );
}

#[test]
fn exit_is_reported_even_if_a_background_process_keeps_the_pipes_open() {
    let d = tempfile::tempdir().unwrap();
    // the leftover sleep inherits stdout/stderr, like a daemon started by an install step
    let exe = stub(d.path(), "sleep 30 &\necho started\nexit 0");
    let c = Child::spawn(&exe, &[], &[]).unwrap();
    let ev = until_exit(&c);
    assert_eq!(ev, [line("started"), ChildEvent::Exited(0)]);
}
