//! Helpers shared by the integration tests that run stub scripts.
#![allow(dead_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// A stub "CLI": a shell script written at run time.
pub fn stub(dir: &Path, body: &str) -> PathBuf {
    let p = dir.join("stub.sh");
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}
