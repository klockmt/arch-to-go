//! Typed install configuration. The TOML file is the single source of truth;
//! the prompt flow and the future TUI build the same struct.
//!
//! No password lives here: unknown keys are rejected, so `password = ...` fails.

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid TOML: {}", short(.0))]
    Parse(#[from] toml::de::Error),
    #[error("invalid config: {0}")]
    Invalid(String),
}

/// The error's first and last lines (where and why), without the quoted source line in between:
/// a file that holds a secret (a `.env`, a `password` key) must not have it echoed.
fn short(e: &toml::de::Error) -> String {
    let text = e.to_string();
    let lines: Vec<&str> = text.lines().collect();
    match (lines.first(), lines.last()) {
        (Some(first), Some(last)) if lines.len() > 1 => format!("{first}: {last}"),
        _ => text,
    }
}

fn bad<T>(msg: impl Into<String>) -> Result<T, ConfigError> {
    Err(ConfigError::Invalid(msg.into()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Fs {
    Btrfs,
    Ext4,
    Xfs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Kernel {
    #[serde(rename = "linux")]
    Linux,
    #[serde(rename = "linux-lts")]
    LinuxLts,
    #[serde(rename = "linux-zen")]
    LinuxZen,
}

impl Kernel {
    /// Package name, also the suffix of `/boot/vmlinuz-*` and `initramfs-*.img`.
    pub fn name(self) -> &'static str {
        match self {
            Kernel::Linux => "linux",
            Kernel::LinuxLts => "linux-lts",
            Kernel::LinuxZen => "linux-zen",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkBackend {
    IwdNetworkd,
    NetworkManager,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Minimal,
    Hyprland,
    Gnome,
    Kde,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DiskCfg {
    /// Optional; the CLI device argument overrides it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub fs: Fs,
    pub esp_size: String,
    /// `None` means the default for the filesystem (see `effective_subvolumes`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subvolumes: Option<Vec<String>>,
    pub nodatacow: bool,
    pub zram: bool,
}

impl Default for DiskCfg {
    fn default() -> Self {
        Self {
            target: None,
            fs: Fs::Btrfs,
            esp_size: "1G".into(),
            subvolumes: None,
            nodatacow: false,
            zram: false,
        }
    }
}

impl DiskCfg {
    pub fn effective_subvolumes(&self) -> Vec<String> {
        match (&self.subvolumes, self.fs) {
            (Some(v), _) => v.clone(),
            (None, Fs::Btrfs) => ["@", "@home", "@log", "@pkg"].map(String::from).to_vec(),
            (None, _) => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct BootCfg {
    pub kernels: Vec<Kernel>,
}

impl Default for BootCfg {
    fn default() -> Self {
        Self {
            kernels: vec![Kernel::Linux],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct SystemCfg {
    pub hostname: String,
    pub locale: String,
    pub keymap: String,
    pub timezone: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mirror_region: Option<String>,
}

impl Default for SystemCfg {
    fn default() -> Self {
        Self {
            hostname: "archtogo".into(),
            locale: "pt_BR.UTF-8".into(),
            keymap: "br-abnt2".into(),
            timezone: "America/Sao_Paulo".into(),
            mirror_region: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UsersCfg {
    pub admin: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProfileCfg {
    pub name: Profile,
    pub audio: bool,
}

impl Default for ProfileCfg {
    fn default() -> Self {
        Self {
            name: Profile::Minimal,
            audio: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct NetworkCfg {
    pub backend: NetworkBackend,
}

impl Default for NetworkCfg {
    fn default() -> Self {
        Self {
            backend: NetworkBackend::IwdNetworkd,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AurCfg {
    pub paru: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PortableCfg {
    pub firmware: bool,
    pub gpu_drivers: bool,
}

impl Default for PortableCfg {
    fn default() -> Self {
        Self {
            firmware: true,
            gpu_drivers: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub disk: DiskCfg,
    #[serde(default)]
    pub boot: BootCfg,
    #[serde(default)]
    pub system: SystemCfg,
    pub users: UsersCfg,
    #[serde(default)]
    pub profile: ProfileCfg,
    #[serde(default)]
    pub network: NetworkCfg,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub aur: AurCfg,
    #[serde(default)]
    pub portable: PortableCfg,
}

/// `true` when `s` is non-empty, at most `max` long and every char passes `ok`.
fn charset(s: &str, max: usize, ok: impl Fn(char) -> bool) -> bool {
    !s.is_empty() && s.len() <= max && s.chars().all(ok)
}

fn is_hostname(s: &str) -> bool {
    charset(s, 63, |c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
    }) && !s.starts_with('-')
}

fn is_username(s: &str) -> bool {
    let first = s.chars().next().unwrap_or(' ');
    (first.is_ascii_lowercase() || first == '_')
        && charset(s, 32, |c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'
        })
}

fn is_timezone(s: &str) -> bool {
    !s.is_empty()
        && s.split('/').all(|p| {
            charset(p, 64, |c| c.is_ascii_alphanumeric() || "_+-".contains(c)) && p != ".."
        })
}

fn is_locale(s: &str) -> bool {
    charset(s, 32, |c| c.is_ascii_alphanumeric() || "_.@-".contains(c)) && s.ends_with("UTF-8")
}

fn is_keymap(s: &str) -> bool {
    charset(s, 32, |c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'
    })
}

fn is_pkg_or_unit(s: &str) -> bool {
    charset(s, 128, |c| c.is_ascii_alphanumeric() || "@._+-".contains(c)) && !s.starts_with('-')
}

fn is_size(s: &str) -> bool {
    let digits = s.trim_end_matches(['M', 'G']);
    digits.len() + 1 == s.len() && !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

fn is_subvolume(s: &str) -> bool {
    s.strip_prefix('@').is_some_and(|r| {
        r.is_empty() || charset(r, 32, |c| c.is_ascii_alphanumeric() || "_-".contains(c))
    })
}

fn no_duplicates<T: PartialEq>(items: &[T]) -> bool {
    items
        .iter()
        .enumerate()
        .all(|(i, a)| !items[..i].contains(a))
}

impl Config {
    pub fn from_toml(s: &str) -> Result<Config, ConfigError> {
        let cfg: Config = toml::from_str(s)?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// The config as TOML, for saving. There is no password field, so none can be written.
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        toml::to_string(self).map_err(|e| ConfigError::Invalid(e.to_string()))
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        let s = &self.system;
        if !is_hostname(&s.hostname) {
            return bad(format!("hostname {:?} is not a valid hostname", s.hostname));
        }
        if !is_username(&self.users.admin) {
            return bad(format!(
                "admin {:?} is not a valid user name",
                self.users.admin
            ));
        }
        if !is_timezone(&s.timezone) {
            return bad(format!(
                "timezone {:?} is not a valid zoneinfo path",
                s.timezone
            ));
        }
        if !is_locale(&s.locale) {
            return bad(format!("locale {:?} must look like pt_BR.UTF-8", s.locale));
        }
        if !is_keymap(&s.keymap) {
            return bad(format!("keymap {:?} is not a valid keymap name", s.keymap));
        }
        if let Some(r) = &s.mirror_region
            && !charset(r, 64, |c| c.is_ascii_alphabetic() || c == ' ')
        {
            return bad(format!("mirror_region {r:?} is not a valid region name"));
        }
        if self.boot.kernels.is_empty() {
            return bad("boot.kernels must list at least one kernel");
        }
        if !no_duplicates(&self.boot.kernels) {
            return bad("boot.kernels has a duplicate");
        }
        if !is_size(&self.disk.esp_size) {
            return bad(format!(
                "disk.esp_size {:?} must look like 512M or 1G",
                self.disk.esp_size
            ));
        }
        if let Some(subs) = &self.disk.subvolumes {
            if self.disk.fs != Fs::Btrfs {
                return bad("disk.subvolumes only applies to btrfs");
            }
            if !subs.iter().any(|v| v == "@") {
                return bad("disk.subvolumes must include \"@\" (the root)");
            }
            if let Some(v) = subs.iter().find(|v| !is_subvolume(v)) {
                return bad(format!("subvolume {v:?} must be @ or @name"));
            }
            if !no_duplicates(subs) {
                return bad("disk.subvolumes has a duplicate");
            }
        }
        for (what, list) in [("package", &self.packages), ("service", &self.services)] {
            if let Some(p) = list.iter().find(|p| !is_pkg_or_unit(p)) {
                return bad(format!("{what} name {p:?} is not valid"));
            }
            if !no_duplicates(list) {
                return bad(format!("duplicate {what} in the list"));
            }
        }
        Ok(())
    }
}
