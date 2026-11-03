use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_arch-to-go");
const PASSWORD: &str = "s3cr3t!Pw";

/// A fake machine: a USB disk `sdz`, with the host running from `nvme0n1`.
fn fixture(tran: &str) -> TempDir {
    let d = TempDir::new().unwrap();
    std::fs::write(d.path().join("disk_info"), format!("sdz disk {tran}\n")).unwrap();
    std::fs::write(d.path().join("host_disks"), "nvme0n1\n").unwrap();
    d
}

fn config(dir: &Path, body: &str) -> String {
    let p = dir.join("c.toml");
    std::fs::write(&p, format!("{body}\n[users]\nadmin = \"tester\"\n")).unwrap();
    p.to_str().unwrap().into()
}

fn run(fix: &TempDir, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .env("ARCH_TO_GO_SYSINFO_DIR", fix.path())
        .env("ARCH_ADMIN_PASSWORD", PASSWORD)
        .env_remove("DRY_RUN")
        .output()
        .unwrap()
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn dry_run_needs_no_root_and_prints_plan() {
    let fix = fixture("usb");
    let cfg = config(fix.path(), "");
    let o = run(
        &fix,
        &["install", "/dev/sdz", "--config", &cfg, "--dry-run"],
    );
    let t = text(&o);
    assert!(o.status.success(), "{t}");
    assert!(t.contains("pacstrap") && t.contains("mkfs.btrfs"), "{t}");
    assert!(t.contains("[dry-run]"), "{t}");
}

#[test]
fn dry_run_env_equals_flag() {
    let fix = fixture("usb");
    let cfg = config(fix.path(), "");
    let o = Command::new(BIN)
        .args(["install", "/dev/sdz", "--config", &cfg])
        .env("ARCH_TO_GO_SYSINFO_DIR", fix.path())
        .env("ARCH_ADMIN_PASSWORD", PASSWORD)
        .env("DRY_RUN", "1")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("[dry-run]"));
}

#[test]
fn dry_run_output_has_no_password() {
    let fix = fixture("usb");
    let cfg = config(fix.path(), "");
    let o = run(
        &fix,
        &["install", "/dev/sdz", "--config", &cfg, "--dry-run"],
    );
    let t = text(&o);
    assert!(o.status.success(), "{t}");
    assert!(
        !t.contains(PASSWORD) && !t.contains("s3cr3t"),
        "password leaked:\n{t}"
    );
    assert!(t.contains("***"), "{t}");
}

#[test]
fn invalid_config_exits_nonzero_before_any_step() {
    let fix = fixture("usb");
    let cfg = config(fix.path(), "[system]\nhostname = \"bad host\"\n");
    let o = run(
        &fix,
        &["install", "/dev/sdz", "--config", &cfg, "--dry-run"],
    );
    assert!(!o.status.success());
    let t = text(&o);
    assert!(t.contains("hostname"), "{t}");
    assert!(!t.contains("[dry-run]"), "{t}");
}

#[test]
fn non_usb_device_refused() {
    let fix = fixture("sata");
    let cfg = config(fix.path(), "");
    let o = run(
        &fix,
        &["install", "/dev/sdz", "--config", &cfg, "--dry-run"],
    );
    assert!(!o.status.success());
    let t = text(&o);
    assert!(t.contains("USB"), "{t}");
    assert!(!t.contains("[dry-run]"), "{t}");
}

#[test]
fn host_disk_refused() {
    let fix = fixture("usb");
    std::fs::write(fix.path().join("host_disks"), "sdz\n").unwrap();
    let cfg = config(fix.path(), "");
    let o = run(
        &fix,
        &["install", "/dev/sdz", "--config", &cfg, "--dry-run"],
    );
    assert!(!o.status.success());
    assert!(
        text(&o).contains("backs the running system"),
        "{}",
        text(&o)
    );
}

#[test]
fn fake_system_is_refused_for_a_real_run() {
    // the fixture must never be able to drive real disk writes
    let fix = fixture("usb");
    let cfg = config(fix.path(), "");
    let o = run(&fix, &["install", "/dev/sdz", "--config", &cfg]);
    assert!(!o.status.success());
    assert!(text(&o).contains("only with --dry-run"), "{}", text(&o));
}

#[test]
fn real_run_requires_root() {
    if std::fs::metadata("/proc/self")
        .map(|m| std::os::unix::fs::MetadataExt::uid(&m))
        .ok()
        == Some(0)
    {
        return; // running as root: a real run would be real
    }
    let d = TempDir::new().unwrap();
    let cfg = config(d.path(), "");
    let o = Command::new(BIN)
        .args(["install", "/dev/sdz", "--config", &cfg])
        .env_remove("ARCH_TO_GO_SYSINFO_DIR")
        .env_remove("DRY_RUN")
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert!(text(&o).contains("root"), "{}", text(&o));
}

#[test]
fn device_argument_overrides_config_target() {
    let fix = fixture("usb");
    let cfg = config(fix.path(), "[disk]\ntarget = \"/dev/other\"\n");
    let o = run(
        &fix,
        &["install", "/dev/sdz", "--config", &cfg, "--dry-run"],
    );
    let t = text(&o);
    assert!(o.status.success(), "{t}");
    assert!(t.contains("/dev/sdz1") && !t.contains("/dev/other"), "{t}");
}

#[test]
fn check_config_accepts_good_and_rejects_bad() {
    let fix = fixture("usb");
    let good = config(fix.path(), "");
    assert!(run(&fix, &["check-config", &good]).status.success());
    let bad = config(fix.path(), "[boot]\nkernels = []\n");
    let o = run(&fix, &["check-config", &bad]);
    assert!(!o.status.success());
    assert!(text(&o).contains("kernel"), "{}", text(&o));
}

#[test]
fn password_in_config_is_rejected() {
    let fix = fixture("usb");
    let p = fix.path().join("c.toml");
    std::fs::write(&p, "[users]\nadmin = \"a\"\npassword = \"x\"\n").unwrap();
    assert!(
        !run(&fix, &["check-config", p.to_str().unwrap()])
            .status
            .success()
    );
}

#[test]
fn loop_device_needs_the_env_flag() {
    let fix = fixture("");
    std::fs::write(fix.path().join("disk_info"), "loop0 loop\n").unwrap();
    let cfg = config(fix.path(), "");
    let args = ["install", "/dev/loop0", "--config", &cfg, "--dry-run"];
    assert!(!run(&fix, &args).status.success());
    let o = Command::new(BIN)
        .args(args)
        .env("ARCH_TO_GO_SYSINFO_DIR", fix.path())
        .env("ARCH_ADMIN_PASSWORD", PASSWORD)
        .env("ARCH_TO_GO_ALLOW_LOOP", "1")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("/dev/loop0p1"), "{}", text(&o));
}

#[test]
fn disk_is_checked_again_right_before_the_wipe() {
    // the stick is swapped while the password or a long paru build is pending:
    // the second inspection sees a SATA disk behind the same name
    let fix = fixture("usb");
    std::fs::write(fix.path().join("disk_info.2"), "sdz disk sata\n").unwrap();
    let cfg = config(fix.path(), "");
    let o = run(
        &fix,
        &["install", "/dev/sdz", "--config", &cfg, "--dry-run"],
    );
    let t = text(&o);
    assert!(!o.status.success(), "{t}");
    assert!(t.contains("USB"), "{t}");
    assert!(
        !t.contains("[dry-run]"),
        "nothing may run after the failed recheck:\n{t}"
    );
}

#[test]
fn tui_refuses_without_a_terminal() {
    let fix = fixture("usb");
    let cfg = config(fix.path(), "");
    let o = run(&fix, &["tui", "--config", &cfg]);
    assert!(!o.status.success());
    assert!(text(&o).contains("needs a terminal"), "{}", text(&o));
    let o = run(&fix, &["tui"]);
    assert!(text(&o).contains("needs a terminal"), "{}", text(&o));
}

#[test]
fn tui_rejects_an_invalid_config_before_opening() {
    let fix = fixture("usb");
    let cfg = config(fix.path(), "[system]\nhostname = \"bad host\"\n");
    let o = run(&fix, &["tui", "--config", &cfg]);
    assert!(!o.status.success());
    let t = text(&o);
    assert!(t.contains("hostname"), "{t}");
    assert!(
        !t.contains("needs a terminal"),
        "the config error comes first:\n{t}"
    );
}

#[test]
fn tui_is_listed_in_help() {
    let o = Command::new(BIN).arg("--help").output().unwrap();
    assert!(text(&o).contains("tui"), "{}", text(&o));
}
