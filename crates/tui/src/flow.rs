//! Review, Confirm, Run and Done screens, and the Save/Load path prompt.

use crate::app::{App, ChildEvent, Effect, Screen};
use crate::progress::{Line, classify};
use arch_to_go_core::config::Config;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    Save,
    Load,
}

pub(crate) struct Prompt {
    kind: PromptKind,
    buf: String,
}

fn ctrl_c(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c')
}

/// Scroll position after Up/Down/PageUp/PageDown; `down_is_plus` flips the direction.
fn scrolled(pos: usize, key: KeyCode, max: usize, down_is_plus: bool) -> usize {
    let (more, less) = match key {
        KeyCode::Down => (1, 0),
        KeyCode::PageDown => (10, 0),
        KeyCode::Up => (0, 1),
        KeyCode::PageUp => (0, 10),
        _ => return pos,
    };
    let (plus, minus) = if down_is_plus {
        (more, less)
    } else {
        (less, more)
    };
    (pos + plus).saturating_sub(minus).min(max)
}

impl App {
    pub fn confirm_text(&self) -> &str {
        &self.confirm
    }

    pub fn prompt(&self) -> Option<(PromptKind, &str)> {
        self.prompt.as_ref().map(|p| (p.kind, p.buf.as_str()))
    }

    pub fn prompt_text(&self) -> Option<&str> {
        self.prompt().map(|(_, t)| t)
    }

    pub(crate) fn open_prompt(&mut self, kind: PromptKind) {
        self.prompt = Some(Prompt {
            kind,
            buf: "config.toml".into(),
        });
    }

    pub(crate) fn prompt_key(&mut self, key: KeyEvent) -> Effect {
        let Some(p) = &mut self.prompt else {
            return Effect::None;
        };
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => p.buf.push(c),
            KeyCode::Backspace => {
                p.buf.pop();
            }
            KeyCode::Esc => self.prompt = None,
            KeyCode::Enter if p.buf.is_empty() => self.message = Some("type a file path".into()),
            KeyCode::Enter => {
                let (kind, path) = (p.kind, PathBuf::from(&p.buf));
                self.prompt = None;
                return match kind {
                    PromptKind::Save => {
                        self.saved_path = Some(path.clone());
                        Effect::Save(path)
                    }
                    PromptKind::Load => Effect::Load(path),
                };
            }
            _ => {}
        }
        Effect::None
    }

    pub(crate) fn try_install(&mut self) {
        if !self.can_install() {
            self.message = Some("not ready: fix the flagged sections first".into());
        } else if !self.is_root {
            let path = self
                .saved_path
                .as_ref()
                .map_or("config.toml".to_owned(), |p| p.display().to_string());
            self.message = Some(format!(
                "root needed: save the config, then run: sudo arch-to-go install {} --config {path}",
                self.device.as_deref().unwrap_or("DEVICE")
            ));
        } else {
            self.confirm.clear();
            self.screen = Screen::Confirm;
        }
    }

    pub fn set_review(&mut self, lines: Vec<String>) {
        self.review = lines;
        self.review_scroll = 0;
        self.screen = Screen::Review;
    }

    /// Replaces the config; the chosen disk and the password are kept.
    pub fn apply_loaded(&mut self, cfg: Config) {
        self.config = cfg;
        self.message = Some("config loaded".into());
    }

    pub fn on_event(&mut self, ev: ChildEvent) {
        match ev {
            ChildEvent::Line(l) => {
                if let Line::Step(s) = classify(&l) {
                    self.step = Some(s);
                }
                self.log.push(l);
            }
            ChildEvent::Exited(code) => {
                self.exit_code = Some(code);
                self.screen = Screen::Done;
            }
        }
    }

    pub(crate) fn review_key(&mut self, key: KeyEvent) -> Effect {
        if key.code == KeyCode::Esc {
            self.screen = Screen::Menu;
        }
        let max = self.review.len().saturating_sub(1);
        self.review_scroll = scrolled(self.review_scroll, key.code, max, true);
        Effect::None
    }

    pub(crate) fn confirm_key(&mut self, key: KeyEvent) -> Effect {
        self.message = None;
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.confirm.push(c)
            }
            KeyCode::Backspace => {
                self.confirm.pop();
            }
            KeyCode::Esc => self.screen = Screen::Menu,
            KeyCode::Enter => {
                // exact match, no trimming: the CLI checks the same thing again
                if self.device.as_deref() == Some(self.confirm.as_str()) {
                    self.log.clear();
                    self.step = None;
                    self.exit_code = None;
                    self.aborting = false;
                    self.log_scroll = 0;
                    self.screen = Screen::Run;
                    return Effect::Install;
                }
                self.message = Some("that is not the exact device path".into());
            }
            _ => {}
        }
        Effect::None
    }

    pub(crate) fn run_key(&mut self, key: KeyEvent) -> Effect {
        if ctrl_c(key) && !self.aborting {
            self.aborting = true;
            self.message = Some("aborting, waiting for cleanup".into());
            return Effect::Abort;
        }
        self.log_scroll = scrolled(self.log_scroll, key.code, self.log.len(), false);
        Effect::None
    }

    pub(crate) fn done_key(&mut self, key: KeyEvent) -> Effect {
        match key.code {
            KeyCode::Char('q') => return Effect::Quit,
            KeyCode::Enter => self.screen = Screen::Menu,
            _ => self.log_scroll = scrolled(self.log_scroll, key.code, self.log.len(), false),
        }
        Effect::None
    }
}
