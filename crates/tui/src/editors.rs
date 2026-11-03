//! Section editors: the rows of each section and how keys change the config. Every edit goes
//! straight into `App::config`; validity is judged by `Config::validate` elsewhere.

use crate::app::{App, Section};
use arch_to_go_core::config::{Fs, Kernel, NetworkBackend, Profile};
use arch_to_go_core::step::Secret;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// One line of a section screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub label: String,
    /// What to show; masked fields only ever hold `*`s.
    pub value: String,
    pub editing: bool,
}

/// A text edit in progress. The value is applied to the config on every key; `original` lets
/// Esc put it back.
pub(crate) struct Edit {
    original: String,
    buf: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Masked,
    Toggle,
    Choice,
}

const KERNELS: [Kernel; 3] = [Kernel::Linux, Kernel::LinuxLts, Kernel::LinuxZen];
const FSES: [Fs; 3] = [Fs::Btrfs, Fs::Ext4, Fs::Xfs];
const PROFILES: [Profile; 4] = [
    Profile::Minimal,
    Profile::Hyprland,
    Profile::Gnome,
    Profile::Kde,
];
const BACKENDS: [NetworkBackend; 2] = [NetworkBackend::IwdNetworkd, NetworkBackend::NetworkManager];

fn row_count(s: Section) -> usize {
    match s {
        Section::Disk | Section::Extras => 5,
        Section::Boot | Section::User => 3,
        Section::System => 4,
        Section::Profile => 2,
        Section::Network => 1,
    }
}

fn kind(s: Section, i: usize) -> Kind {
    match (s, i) {
        (Section::Disk, 0 | 1) | (Section::Profile, 0) | (Section::Network, _) => Kind::Choice,
        (Section::Disk, 2)
        | (Section::System, _)
        | (Section::User, 0)
        | (Section::Extras, 0 | 1) => Kind::Text,
        (Section::User, _) => Kind::Masked,
        _ => Kind::Toggle,
    }
}

fn check(on: bool) -> String {
    if on { "[x]" } else { "[ ]" }.into()
}

/// The next (or previous) element of `all`, wrapping; `cur` not found starts at the edge.
fn cycle<T: Copy + PartialEq>(all: &[T], cur: T, dir: i32) -> T {
    let i = all.iter().position(|x| *x == cur).unwrap_or(0) as i32;
    all[(i + dir).rem_euclid(all.len() as i32) as usize]
}

impl App {
    /// The rows of a section, for drawing and tests.
    pub fn rows(&self, s: Section) -> Vec<Row> {
        let c = &self.config;
        let raw: Vec<(&str, String)> = match s {
            Section::Disk => vec![
                (
                    "Device",
                    self.device.clone().unwrap_or_else(|| "none".into()),
                ),
                ("Filesystem", format!("{:?}", c.disk.fs).to_lowercase()),
                ("ESP size", c.disk.esp_size.clone()),
                ("No CoW (nodatacow)", check(c.disk.nodatacow)),
                ("zram swap", check(c.disk.zram)),
            ],
            Section::Boot => KERNELS
                .iter()
                .map(|k| (k.name(), check(c.boot.kernels.contains(k))))
                .collect(),
            Section::System => vec![
                ("Hostname", c.system.hostname.clone()),
                ("Locale", c.system.locale.clone()),
                ("Keymap", c.system.keymap.clone()),
                ("Timezone", c.system.timezone.clone()),
            ],
            Section::User => vec![
                ("Admin name", c.users.admin.clone()),
                ("Password", self.password_display()),
                ("Repeat password", self.repeat_display()),
            ],
            Section::Profile => vec![
                ("Profile", format!("{:?}", c.profile.name).to_lowercase()),
                ("Audio (pipewire)", check(c.profile.audio)),
            ],
            Section::Network => vec![("Backend", format!("{:?}", c.network.backend))],
            Section::Extras => vec![
                ("Packages", c.packages.join(" ")),
                ("Services", c.services.join(" ")),
                ("AUR helper (paru)", check(c.aur.paru)),
                ("Firmware and microcode", check(c.portable.firmware)),
                ("GPU drivers", check(c.portable.gpu_drivers)),
            ],
        };
        raw.into_iter()
            .enumerate()
            .map(|(i, (label, value))| {
                let editing = self.cursor == i && self.edit.is_some();
                let value = match (&self.edit, editing, kind(s, i)) {
                    (Some(e), true, Kind::Text) => e.buf.clone(),
                    _ => value,
                };
                Row {
                    label: label.into(),
                    value,
                    editing,
                }
            })
            .collect()
    }

    /// The selected row of the open section.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// `*` per character of the password being typed, never the characters.
    pub fn password_display(&self) -> String {
        "*".repeat(self.pw_first.chars().count())
    }

    pub fn repeat_display(&self) -> String {
        "*".repeat(self.pw_repeat.chars().count())
    }

    fn get_text(&self, s: Section, i: usize) -> String {
        let c = &self.config;
        match (s, i) {
            (Section::Disk, _) => c.disk.esp_size.clone(),
            (Section::System, 0) => c.system.hostname.clone(),
            (Section::System, 1) => c.system.locale.clone(),
            (Section::System, 2) => c.system.keymap.clone(),
            (Section::System, _) => c.system.timezone.clone(),
            (Section::User, 0) => c.users.admin.clone(),
            (Section::User, 1) => self.pw_first.clone(),
            (Section::User, _) => self.pw_repeat.clone(),
            (Section::Extras, 0) => c.packages.join(" "),
            _ => c.services.join(" "),
        }
    }

    fn set_text(&mut self, s: Section, i: usize, v: &str) {
        let words = |v: &str| v.split_whitespace().map(String::from).collect::<Vec<_>>();
        let c = &mut self.config;
        match (s, i) {
            (Section::Disk, _) => c.disk.esp_size = v.into(),
            (Section::System, 0) => c.system.hostname = v.into(),
            (Section::System, 1) => c.system.locale = v.into(),
            (Section::System, 2) => c.system.keymap = v.into(),
            (Section::System, _) => c.system.timezone = v.into(),
            (Section::User, 0) => c.users.admin = v.into(),
            (Section::User, 1) => {
                self.pw_first = v.into();
                self.update_password();
            }
            (Section::User, _) => {
                self.pw_repeat = v.into();
                self.update_password();
            }
            (Section::Extras, 0) => c.packages = words(v),
            _ => c.services = words(v),
        }
    }

    /// The password counts only when both entries match and are not empty.
    fn update_password(&mut self) {
        if !self.pw_first.is_empty() && self.pw_first == self.pw_repeat {
            self.password = Some(Secret::new(self.pw_first.clone()));
        } else {
            self.password = None;
            if !self.pw_first.is_empty() && !self.pw_repeat.is_empty() {
                self.message = Some("the two passwords differ".into());
            }
        }
    }

    pub(crate) fn section_key(&mut self, s: Section, key: KeyEvent) {
        self.message = None;
        if self.edit.is_some() {
            self.edit_key(s, key);
            return;
        }
        match key.code {
            KeyCode::Esc => {
                self.screen = crate::app::Screen::Menu;
                self.cursor = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                self.cursor = (self.cursor + 1).min(row_count(s) - 1);
            }
            KeyCode::Left if kind(s, self.cursor) == Kind::Choice => self.cycle_row(s, -1),
            KeyCode::Right if kind(s, self.cursor) == Kind::Choice => self.cycle_row(s, 1),
            KeyCode::Char(' ') if kind(s, self.cursor) == Kind::Toggle => self.toggle_row(s),
            KeyCode::Enter => match kind(s, self.cursor) {
                Kind::Text | Kind::Masked => {
                    let v = self.get_text(s, self.cursor);
                    self.edit = Some(Edit {
                        original: v.clone(),
                        buf: v,
                    });
                }
                Kind::Toggle => self.toggle_row(s),
                Kind::Choice => self.cycle_row(s, 1),
            },
            _ => {}
        }
    }

    fn edit_key(&mut self, s: Section, key: KeyEvent) {
        let Some(edit) = &mut self.edit else { return };
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => edit.buf.push(c),
            KeyCode::Backspace => {
                edit.buf.pop();
            }
            KeyCode::Enter => {
                self.edit = None;
                if s == Section::User {
                    self.update_password(); // the key press cleared the message: show it again
                }
                return;
            }
            KeyCode::Esc => {
                let original = edit.original.clone();
                self.edit = None;
                self.set_text(s, self.cursor, &original);
                return;
            }
            _ => return,
        }
        let buf = edit.buf.clone();
        self.set_text(s, self.cursor, &buf);
    }

    fn cycle_row(&mut self, s: Section, dir: i32) {
        match (s, self.cursor) {
            (Section::Disk, 0) => {
                if self.disks.is_empty() {
                    self.message = Some("no USB disk found".into());
                    return;
                }
                let at = self
                    .device
                    .as_ref()
                    .and_then(|d| self.disks.iter().position(|x| &x.name == d));
                let n = self.disks.len() as i32;
                let next = match at {
                    Some(i) => (i as i32 + dir).rem_euclid(n),
                    None if dir > 0 => 0,
                    None => n - 1,
                };
                self.device = Some(self.disks[next as usize].name.clone());
            }
            (Section::Disk, _) => {
                self.config.disk.fs = cycle(&FSES, self.config.disk.fs, dir);
                if self.config.disk.fs != Fs::Btrfs {
                    self.config.disk.subvolumes = None; // validation rejects them on other fs
                }
            }
            (Section::Profile, _) => {
                self.config.profile.name = cycle(&PROFILES, self.config.profile.name, dir)
            }
            (Section::Network, _) => {
                self.config.network.backend = cycle(&BACKENDS, self.config.network.backend, dir)
            }
            _ => {}
        }
    }

    fn toggle_row(&mut self, s: Section) {
        let c = &mut self.config;
        match (s, self.cursor) {
            (Section::Disk, 3) => c.disk.nodatacow = !c.disk.nodatacow,
            (Section::Disk, _) => c.disk.zram = !c.disk.zram,
            (Section::Boot, i) => {
                let k = KERNELS[i];
                let mut sel: Vec<Kernel> = c.boot.kernels.clone();
                if sel.contains(&k) {
                    if sel.len() == 1 {
                        self.message = Some("keep at least one kernel".into());
                        return;
                    }
                    sel.retain(|x| *x != k);
                } else {
                    sel.push(k);
                }
                // canonical order, so the first entry (the default boot) is predictable
                c.boot.kernels = KERNELS
                    .iter()
                    .copied()
                    .filter(|x| sel.contains(x))
                    .collect();
            }
            (Section::Profile, _) => c.profile.audio = !c.profile.audio,
            (Section::Extras, 2) => c.aur.paru = !c.aur.paru,
            (Section::Extras, 3) => c.portable.firmware = !c.portable.firmware,
            (Section::Extras, _) => c.portable.gpu_drivers = !c.portable.gpu_drivers,
            _ => {}
        }
    }
}
