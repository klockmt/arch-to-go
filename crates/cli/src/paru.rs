//! paru is built on the host as a normal user (makepkg refuses root) and the
//! package is installed into the target with `pacman -U`.

use arch_to_go_core::runner::child_env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Home directory of `user` from /etc/passwd text.
fn home_of(passwd: &str, user: &str) -> Option<String> {
    passwd.lines().find_map(|l| {
        let f: Vec<&str> = l.split(':').collect();
        (f.first() == Some(&user) && f.len() > 5).then(|| f[5].to_owned())
    })
}

fn run(user: &str, dir: Option<&Path>, argv: &[&str]) -> Result<(), String> {
    let passwd = std::fs::read_to_string("/etc/passwd").map_err(|e| e.to_string())?;
    let home = home_of(&passwd, user).ok_or_else(|| format!("no home directory for {user}"))?;
    let mut c = Command::new("runuser");
    c.args(["-u", user, "--"]).args(argv);
    // no password variable may reach the downloaded PKGBUILD; HOME is the user's, not root's
    c.env_clear()
        .envs(child_env(std::env::vars()))
        .env("HOME", home);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_is_read_from_passwd() {
        let passwd = "root:x:0:0::/root:/bin/bash\nbob:x:1000:1000:Bob:/home/bob:/bin/fish\n";
        assert_eq!(home_of(passwd, "bob").as_deref(), Some("/home/bob"));
        assert_eq!(home_of(passwd, "bo"), None);
    }
}
