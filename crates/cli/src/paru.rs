//! paru is built on the host as a normal user (makepkg refuses root) and the
//! package is installed into the target with `pacman -U`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn run(user: &str, dir: Option<&Path>, argv: &[&str]) -> Result<(), String> {
    let mut c = Command::new("runuser");
    c.args(["-u", user, "--"]).args(argv);
    if let Some(d) = dir {
        c.current_dir(d);
    }
    match c.status() {
        Ok(s) if s.success() => Ok(()),
        _ => Err(format!("`{}` failed", argv.join(" "))),
    }
}

fn find_pkg(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            n.starts_with("paru-") && !n.starts_with("paru-debug-") && n.ends_with(".pkg.tar.zst")
        })
}

/// Returns the path of the package, reusing a cached one in `pkgs/`.
pub fn build() -> Result<String, String> {
    let work = std::env::current_dir().map_err(|e| e.to_string())?;
    let cache = work.join("pkgs");
    if let Some(p) = find_pkg(&cache) {
        return Ok(p.to_string_lossy().into_owned());
    }
    let user = std::env::var("SUDO_USER").unwrap_or_default();
    if user.is_empty() || user == "root" {
        return Err("aur.paru: run with sudo from a normal user; makepkg refuses root".into());
    }
    let build = work.join("build");
    let src = build.join("paru");
    run(&user, None, &["mkdir", "-p", &build.to_string_lossy()])?;
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&src);
    run(
        &user,
        None,
        &[
            "git",
            "clone",
            "--depth",
            "1",
            "https://aur.archlinux.org/paru.git",
            &src.to_string_lossy(),
        ],
    )?;
    run(&user, Some(&src), &["makepkg", "--noconfirm"])?;
    let pkg = find_pkg(&src).ok_or("makepkg produced no paru package")?;
    let dest = cache.join(pkg.file_name().ok_or("bad package name")?);
    std::fs::copy(&pkg, &dest).map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().into_owned())
}
