use arch_to_go_core::exit_guard::{MountGuard, Unmount};
use arch_to_go_core::runner::{DryRunner, MockRunner, Runner, execute};
use arch_to_go_core::step::{Cmd, Secret, Step};
use std::cell::RefCell;
use std::rc::Rc;

fn run_step(desc: &str, argv: &[&str]) -> Step {
    Step::Run {
        desc: desc.into(),
        cmd: Cmd::new(argv),
    }
}

#[test]
fn dry_run_never_prints_stdin_secret() {
    let mut out = Vec::new();
    let cmd =
        Cmd::new(&["arch-chroot", "/mnt", "chpasswd"]).with_stdin(Secret::new("tester:p:a'ss $x"));
    let steps = [Step::Run {
        desc: "set password".into(),
        cmd,
    }];
    execute(&steps, &mut DryRunner::new(&mut out)).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("chpasswd"), "{text}");
    assert!(text.contains("***"), "{text}");
    assert!(!text.contains("a'ss"), "{text}");
}

#[test]
fn secret_debug_and_display_are_redacted() {
    let s = Secret::new("hunter2");
    assert_eq!(format!("{s:?}"), "***");
    assert_eq!(format!("{s}"), "***");
    assert_eq!(s.expose(), "hunter2");
    let cmd = Cmd::new(&["x"]).with_stdin(s);
    assert!(!format!("{cmd:?}").contains("hunter2"));
}

#[test]
fn execute_stops_at_first_failure() {
    let mut m = MockRunner {
        fail_at: Some(1),
        ..Default::default()
    };
    let steps = [
        run_step("a", &["one"]),
        run_step("b", &["two"]),
        run_step("c", &["three"]),
    ];
    assert!(execute(&steps, &mut m).is_err());
    assert_eq!(
        m.cmds.len(),
        2,
        "the failing call is recorded, the next is not"
    );
}

#[test]
fn write_file_steps_reach_the_runner() {
    let mut m = MockRunner::default();
    let steps = [Step::WriteFile {
        desc: "hostname".into(),
        path: "/mnt/etc/hostname".into(),
        contents: "box\n".into(),
        mode: 0o644,
    }];
    execute(&steps, &mut m).unwrap();
    assert_eq!(m.files, vec![("/mnt/etc/hostname".into(), "box\n".into())]);
}

#[test]
fn guard_unmounts_on_drop_after_failure() {
    let m = Rc::new(RefCell::new(MockRunner {
        fail_at: Some(0),
        ..Default::default()
    }));
    {
        let _guard = MountGuard::new(m.clone());
        let steps = [run_step("boom", &["false"])];
        assert!(execute(&steps, &mut *m.borrow_mut()).is_err());
        m.borrow_mut().fail_at = None;
    }
    let cmds = &m.borrow().cmds;
    assert_eq!(cmds.last().unwrap().argv, ["umount", "-R", "/mnt"]);
}

#[test]
fn guard_falls_back_to_lazy_unmount() {
    let m = Rc::new(RefCell::new(MockRunner {
        fail_at: Some(1), // call 0 stops the agent, call 1 is the first umount
        ..Default::default()
    }));
    drop(MountGuard::new(m.clone()));
    let cmds = &m.borrow().cmds;
    assert_eq!(cmds.len(), 3);
    assert_eq!(cmds[2].argv, ["umount", "-R", "-l", "/mnt"]);
}

#[test]
fn mock_is_a_runner() {
    // compile-time check that the trait is object safe
    let _: &mut dyn Runner = &mut MockRunner::default();
}

#[test]
fn real_runner_reports_nonzero_exit() {
    let err = arch_to_go_core::runner::RealRunner
        .run(&Cmd::new(&["false"]))
        .unwrap_err();
    assert!(err.to_string().contains("false"), "{err}");
}

#[test]
fn real_runner_delivers_stdin_verbatim() {
    // the shell here is only the test's probe; the runner itself uses no shell
    let probe = Cmd::new(&["sh", "-c", r#"test "$(cat)" = "u:p:a'b \$x""#])
        .with_stdin(Secret::new("u:p:a'b $x"));
    arch_to_go_core::runner::RealRunner.run(&probe).unwrap();
}

#[test]
fn child_env_keeps_only_an_allow_list() {
    use arch_to_go_core::runner::child_env;
    let parent = [
        ("PATH", "/usr/bin"),
        ("LANG", "C"),
        ("ARCH_ADMIN_PASSWORD", "x"),
        ("ARCH_SUDO_PASSWORD", "y"),
        ("GITHUB_TOKEN", "z"),
        ("SSH_AUTH_SOCK", "/s"),
    ]
    .map(|(k, v)| (k.to_string(), v.to_string()));
    let kept: Vec<String> = child_env(parent.into_iter())
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    assert_eq!(kept, ["PATH", "LANG"]);
}

#[test]
fn finish_reports_a_clean_unmount_and_does_not_unmount_twice() {
    let m = Rc::new(RefCell::new(MockRunner::default()));
    let guard = MountGuard::new(m.clone());
    assert_eq!(guard.finish(), Unmount::Clean);
    assert_eq!(
        m.borrow().cmds.len(),
        2,
        "agent stop + umount; drop must not run them again"
    );
}

#[test]
fn finish_reports_a_lazy_unmount() {
    let m = Rc::new(RefCell::new(MockRunner {
        fail_at: Some(1),
        ..Default::default()
    }));
    assert_eq!(MountGuard::new(m.clone()).finish(), Unmount::Lazy);
}

#[test]
fn finish_reports_a_failed_unmount() {
    // both attempts fail: the caller must not say "installed"
    struct AlwaysFails;
    impl Runner for AlwaysFails {
        fn run(&mut self, _: &Cmd) -> Result<(), arch_to_go_core::runner::RunError> {
            Err(arch_to_go_core::runner::RunError::Empty)
        }
        fn write_file(
            &mut self,
            _: &str,
            _: &str,
            _: u32,
        ) -> Result<(), arch_to_go_core::runner::RunError> {
            Ok(())
        }
    }
    let r = Rc::new(RefCell::new(AlwaysFails));
    assert_eq!(MountGuard::new(r).finish(), Unmount::Failed);
}

#[test]
fn guard_stops_the_gpg_agent_before_unmounting() {
    // an interrupted or failed install never reaches the plan's own gpgconf step; the agent
    // pacstrap started would keep /mnt busy, leaving a lazy unmount and an attached loop device
    let m = Rc::new(RefCell::new(MockRunner::default()));
    drop(MountGuard::new(m.clone()));
    let cmds = &m.borrow().cmds;
    assert_eq!(
        cmds[0].argv,
        [
            "gpgconf",
            "--homedir",
            "/mnt/etc/pacman.d/gnupg",
            "--kill",
            "all"
        ]
    );
    assert_eq!(cmds[1].argv, ["umount", "-R", "/mnt"]);
}

#[test]
fn a_failing_agent_stop_does_not_block_the_unmount() {
    let m = Rc::new(RefCell::new(MockRunner {
        fail_at: Some(0),
        ..Default::default()
    }));
    assert_eq!(MountGuard::new(m.clone()).finish(), Unmount::Clean);
    assert_eq!(m.borrow().cmds[1].argv, ["umount", "-R", "/mnt"]);
}
