//! Helpers shared by the integration tests that run stub scripts.
#![allow(dead_code)]

use arch_to_go_tui::driver::Child;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A stub "CLI": a shell script written at run time.
pub fn stub(dir: &Path, body: &str) -> PathBuf {
    let p = dir.join("stub.sh");
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

/// `Child::spawn`, retried on ETXTBSY: while one test thread writes its stub, another thread's
/// fork can briefly hold the write handle, and exec then fails with "Text file busy". Only
/// test scripts are written and run in the same process, so production never sees this.
pub fn spawn(exe: &Path, args: &[String], env: &[(String, String)]) -> Result<Child, String> {
    let mut last = String::new();
    for _ in 0..50 {
        match Child::spawn(exe, args, env) {
            Err(e) if e.contains("Text file busy") => {
                last = e;
                std::thread::sleep(Duration::from_millis(20));
            }
            other => return other,
        }
    }
    Err(last)
}
