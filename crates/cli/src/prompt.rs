//! Interactive input: the config questions and the hidden password.

use arch_to_go_core::config::Config;
use std::io::{BufRead, Write};

pub fn line(question: &str, default: &str) -> Result<String, String> {
    let mut out = std::io::stderr();
    if default.is_empty() {
        write!(out, "{question}: ").ok();
    } else {
        write!(out, "{question} [{default}]: ").ok();
    }
    out.flush().ok();
    let mut s = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut s)
        .map_err(|e| e.to_string())?;
    let s = s.trim();
    Ok(if s.is_empty() {
        default.into()
    } else {
        s.into()
    })
}

/// Asks twice, hidden; loops until both match and are non-empty.
pub fn password() -> Result<String, String> {
    loop {
        let a =
            rpassword::prompt_password("Password (also root's): ").map_err(|e| e.to_string())?;
        let b = rpassword::prompt_password("Repeat the password: ").map_err(|e| e.to_string())?;
        if !a.is_empty() && a == b {
            return Ok(a);
        }
        eprintln!("empty or different, try again");
    }
}

/// The prompt flow builds the same TOML a file would hold, so validation is shared.
pub fn ask_config() -> Result<Config, String> {
    let admin = line("Admin user name", "")?;
    let hostname = line("Hostname", "archtogo")?;
    let locale = line("Locale", "pt_BR.UTF-8")?;
    let keymap = line("Console keymap", "br-abnt2")?;
    let timezone = line("Timezone", "America/Sao_Paulo")?;
    let kernels = line(
        "Kernels (linux, linux-lts, linux-zen; comma separated)",
        "linux",
    )?;
    let profile = line("Profile (minimal, hyprland, gnome, kde)", "minimal")?;
    let network = line("Network (iwd-networkd, network-manager)", "iwd-networkd")?;

    let table = |pairs: Vec<(&str, toml::Value)>| {
        toml::Value::Table(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
    };
    let s = |v: String| toml::Value::String(v);
    let kernel_list = kernels
        .split(',')
        .map(|k| s(k.trim().to_owned()))
        .collect::<Vec<_>>();
    let mut root = toml::Table::new();
    root.insert("users".into(), table(vec![("admin", s(admin))]));
    root.insert(
        "system".into(),
        table(vec![
            ("hostname", s(hostname)),
            ("locale", s(locale)),
            ("keymap", s(keymap)),
            ("timezone", s(timezone)),
        ]),
    );
    root.insert(
        "boot".into(),
        table(vec![("kernels", toml::Value::Array(kernel_list))]),
    );
    root.insert("profile".into(), table(vec![("name", s(profile))]));
    root.insert("network".into(), table(vec![("backend", s(network))]));
    Config::from_toml(&toml::to_string(&root).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
