//! The TUI's state machine. Pure: keys and child events in, state and effects out, so it is
//! tested without a terminal. The runtime performs the effects.

use crate::disks::Disk;
use crate::editors::Edit;
use crate::flow::Prompt;
use arch_to_go_core::config::Config;
use arch_to_go_core::step::Secret;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Disk,
    Boot,
    System,
    User,
    Profile,
    Network,
    Extras,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Menu,
    Section(Section),
    Review,
    Confirm,
    Run,
    Done,
}

/// What the runtime must do after a key press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    Quit,
    Review,
    Install,
    Abort,
    Save(PathBuf),
    Load(PathBuf),
}

/// What the child process reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildEvent {
    Line(String),
    Exited(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    pub label: String,
    pub value: String,
    pub invalid: bool,
}

#[derive(Clone, Copy)]
enum Action {
    Open(Section),
    Save,
    Load,
    Review,
    Install,
    Quit,
}

const MENU: [(&str, Action); 12] = [
    ("Disk", Action::Open(Section::Disk)),
    ("Boot", Action::Open(Section::Boot)),
    ("System", Action::Open(Section::System)),
    ("User", Action::Open(Section::User)),
    ("Profile", Action::Open(Section::Profile)),
    ("Network", Action::Open(Section::Network)),
    ("Extras", Action::Open(Section::Extras)),
    ("Save", Action::Save),
    ("Load", Action::Load),
    ("Review", Action::Review),
    ("Install", Action::Install),
    ("Quit", Action::Quit),
];

pub struct App {
    pub screen: Screen,
    pub config: Config,
    pub device: Option<String>,
    pub password: Option<Secret>,
    pub is_root: bool,
    pub disks: Vec<Disk>,
    pub review: Vec<String>,
    pub log: Vec<String>,
    pub step: Option<String>,
    pub exit_code: Option<i32>,
    pub message: Option<String>,
    /// Lines scrolled down from the top of the plan preview.
    pub review_scroll: usize,
    /// Lines scrolled up from the end of the log.
    pub log_scroll: usize,
    pub(crate) confirm: String,
    pub(crate) prompt: Option<Prompt>,
    pub(crate) aborting: bool,
    pub(crate) saved_path: Option<PathBuf>,
    menu_cursor: usize,
    // section editor state (see editors.rs)
    pub(crate) cursor: usize,
    pub(crate) edit: Option<Edit>,
    pub(crate) pw_first: String,
    pub(crate) pw_repeat: String,
}

/// A valid config with an empty admin name, so a fresh app starts invalid.
fn blank_config() -> Config {
    let mut c = Config::from_toml("[users]\nadmin = \"x\"\n").expect("baseline config");
    c.users.admin = String::new();
    c
}

impl App {
    pub fn new(initial: Option<Config>, is_root: bool, disks: Vec<Disk>) -> App {
        App {
            screen: Screen::Menu,
            config: initial.unwrap_or_else(blank_config),
            device: None,
            password: None,
            is_root,
            disks,
            review: Vec::new(),
            log: Vec::new(),
            step: None,
            exit_code: None,
            message: None,
            review_scroll: 0,
            log_scroll: 0,
            confirm: String::new(),
            prompt: None,
            aborting: false,
            saved_path: None,
            menu_cursor: 0,
            cursor: 0,
            edit: None,
            pw_first: String::new(),
            pw_repeat: String::new(),
        }
    }

    /// The chosen device with its size and model, e.g. `/dev/sda (111,8G KINGSTON)`: the path
    /// alone does not tell a backup stick from the target. `None` until a disk is picked.
    pub fn device_label(&self) -> Option<String> {
        let dev = self.device.as_ref()?;
        Some(match self.disks.iter().find(|d| &d.name == dev) {
            Some(d) => format!("{dev} ({} {})", d.size, d.model).replace(" )", ")"),
            None => dev.clone(),
        })
    }

    /// The selected menu entry.
    pub fn menu_cursor(&self) -> usize {
        self.menu_cursor
    }

    /// Valid config, a disk and a password. The CLI still checks the disk itself.
    pub fn can_install(&self) -> bool {
        self.config.validate().is_ok() && self.device.is_some() && self.password.is_some()
    }

    /// Why a section is invalid, found by validating only that section against a known-good
    /// baseline, so the TUI holds no copy of the rules and one bad section flags only itself.
    fn section_error(&self, s: Section) -> Option<String> {
        let mut c = Config::from_toml("[users]\nadmin = \"x\"\n").expect("baseline config");
        match s {
            Section::Disk => {
                c.disk = self.config.disk.clone();
                if self.device.is_none() {
                    return Some(if self.disks.is_empty() {
                        "no USB disk found".into()
                    } else {
                        "pick a disk".into()
                    });
                }
            }
            Section::Boot => c.boot = self.config.boot.clone(),
            Section::System => c.system = self.config.system.clone(),
            Section::User => {
                c.users = self.config.users.clone();
                if self.password.is_none() && c.validate().is_ok() {
                    return Some("no password set".into());
                }
            }
            Section::Extras => {
                c.packages = self.config.packages.clone();
                c.services = self.config.services.clone();
            }
            Section::Profile | Section::Network => return None,
        }
        c.validate().err().map(|e| e.to_string())
    }

    pub fn menu_items(&self) -> Vec<MenuItem> {
        let c = &self.config;
        MENU.iter()
            .map(|(label, action)| {
                let (value, invalid) = match action {
                    Action::Open(s) => {
                        let value = match s {
                            Section::Disk => match (&self.device, self.disks.is_empty()) {
                                (Some(_), _) => self.device_label().unwrap_or_default(),
                                (None, true) => "no USB disk found".into(),
                                (None, false) => "not selected".into(),
                            },
                            Section::Boot => c
                                .boot
                                .kernels
                                .iter()
                                .map(|k| k.name())
                                .collect::<Vec<_>>()
                                .join(", "),
                            Section::System => c.system.hostname.clone(),
                            Section::User => format!(
                                "{} ({})",
                                c.users.admin,
                                if self.password.is_some() {
                                    "password set"
                                } else {
                                    "no password"
                                }
                            ),
                            Section::Profile => format!("{:?}", c.profile.name).to_lowercase(),
                            Section::Network => format!("{:?}", c.network.backend),
                            Section::Extras => {
                                format!(
                                    "{} packages, {} services",
                                    c.packages.len(),
                                    c.services.len()
                                )
                            }
                        };
                        (value, self.section_error(*s).is_some())
                    }
                    Action::Install => {
                        let v = if !self.is_root {
                            "needs root"
                        } else if self.can_install() {
                            "ready"
                        } else {
                            "not ready"
                        };
                        (v.into(), false)
                    }
                    _ => (String::new(), false),
                };
                MenuItem {
                    label: (*label).into(),
                    value,
                    invalid,
                }
            })
            .collect()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Effect {
        match self.screen {
            Screen::Menu => self.menu_key(key),
            Screen::Section(s) => {
                self.section_key(s, key);
                Effect::None
            }
            Screen::Review => self.review_key(key),
            Screen::Confirm => self.confirm_key(key),
            Screen::Run => self.run_key(key),
            Screen::Done => self.done_key(key),
        }
    }

    fn menu_key(&mut self, key: KeyEvent) -> Effect {
        if self.prompt.is_some() {
            return self.prompt_key(key);
        }
        self.message = None;
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Effect::Quit;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.menu_cursor = self.menu_cursor.saturating_sub(1);
                Effect::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.menu_cursor = (self.menu_cursor + 1).min(MENU.len() - 1);
                Effect::None
            }
            KeyCode::Char('q') => Effect::Quit,
            KeyCode::Enter => match MENU[self.menu_cursor].1 {
                Action::Open(s) => {
                    self.cursor = 0;
                    self.screen = Screen::Section(s);
                    Effect::None
                }
                Action::Review => {
                    if self.config.validate().is_ok() && self.device.is_some() {
                        Effect::Review
                    } else {
                        self.message =
                            Some("fix the flagged sections and pick a disk first".into());
                        Effect::None
                    }
                }
                Action::Install => {
                    self.try_install();
                    Effect::None
                }
                Action::Quit => Effect::Quit,
                Action::Save => {
                    self.open_prompt(crate::flow::PromptKind::Save);
                    Effect::None
                }
                Action::Load => {
                    self.open_prompt(crate::flow::PromptKind::Load);
                    Effect::None
                }
            },
            _ => Effect::None,
        }
    }
}
