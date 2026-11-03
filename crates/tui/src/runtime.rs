//! Performs the app's effects: runs the CLI as a child, saves and loads configs, and owns the
//! terminal loop. The CLI does all the real work and all the safety checks.

use crate::app::{App, ChildEvent, Effect};
use crate::disks::list_usb_disks;
use crate::driver::{Child, install_args};
use crate::ui::draw;
use arch_to_go_core::config::Config;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use std::io::IsTerminal;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};
use tempfile::TempDir;

pub struct Runtime {
    exe: PathBuf,
    /// How long the plan preview may run before it is interrupted.
    pub review_timeout: Duration,
    /// Private directory holding the config file the child reads; removed when the child ends.
    tmp: Option<TempDir>,
    child: Option<Child>,
}

impl Runtime {
    pub fn new(exe: PathBuf) -> Runtime {
        Runtime {
            exe,
            review_timeout: Duration::from_secs(30),
            tmp: None,
            child: None,
        }
    }

    /// The temp directory while a config file exists.
    pub fn config_dir(&self) -> Option<&Path> {
        self.tmp.as_ref().map(|t| t.path())
    }

    /// Writes the config (no secrets: the type has no password field) to a 0700 directory.
    fn write_config(&mut self, app: &App) -> Result<PathBuf, String> {
        // tempfile makes directories with the default umask (755): ask for 0700 explicitly
        let dir = tempfile::Builder::new()
            .prefix("arch-to-go-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir()
            .map_err(|e| format!("cannot create a temp directory: {e}"))?;
        let path = dir.path().join("config.toml");
        let text = app.config.to_toml().map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        self.tmp = Some(dir);
        Ok(path)
    }

    /// The password reaches the child only through its environment.
    fn env(app: &App) -> Vec<(String, String)> {
        app.password
            .iter()
            .map(|p| ("ARCH_ADMIN_PASSWORD".to_owned(), p.expose().to_owned()))
            .collect()
    }

    pub fn perform(&mut self, effect: Effect, app: &mut App) {
        match effect {
            Effect::None | Effect::Quit => {}
            Effect::Review => self.review(app),
            Effect::Install => self.install(app),
            Effect::Abort => {
                if let Some(c) = &self.child {
                    c.interrupt();
                }
            }
            Effect::Save(path) => {
                app.message = Some(match app.config.to_toml() {
                    Err(e) => format!("cannot save {}: {e}", path.display()),
                    Ok(text) => match std::fs::write(&path, text) {
                        Ok(()) => format!("saved to {}", path.display()),
                        Err(e) => format!("cannot save {}: {e}", path.display()),
                    },
                });
            }
            Effect::Load(path) => {
                let loaded = std::fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|t| Config::from_toml(&t).map_err(|e| e.to_string()));
                match loaded {
                    Ok(cfg) => app.apply_loaded(cfg),
                    Err(e) => app.message = Some(format!("cannot load {}: {e}", path.display())),
                }
            }
        }
    }

    /// Runs `install --dry-run` and waits for it: it only prints the plan.
    fn review(&mut self, app: &mut App) {
        let Some(device) = app.device.clone() else {
            app.message = Some("pick a disk first".into());
            return;
        };
        let result = self.write_config(app).and_then(|cfg| {
            Child::spawn(
                &self.exe,
                &install_args(&device, &cfg, true),
                &Self::env(app),
            )
        });
        let child = match result {
            Ok(c) => c,
            Err(e) => {
                self.tmp = None;
                app.message = Some("the preview could not start".into());
                app.set_review(vec![e]);
                return;
            }
        };
        let mut lines = Vec::new();
        let deadline = Instant::now() + self.review_timeout;
        let mut timed_out = false;
        let code = loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match child.rx.recv_timeout(left) {
                Ok(ChildEvent::Line(l)) => lines.push(l),
                Ok(ChildEvent::Exited(c)) => break c,
                Err(RecvTimeoutError::Timeout) => {
                    // a dry run that hangs (lsblk on a dead device) must not freeze the screen
                    timed_out = true;
                    child.interrupt();
                    let end = Instant::now() + Duration::from_secs(5);
                    while let Ok(ChildEvent::Line(l)) = child
                        .rx
                        .recv_timeout(end.saturating_duration_since(Instant::now()))
                    {
                        lines.push(l);
                    }
                    break 124;
                }
                Err(RecvTimeoutError::Disconnected) => break 127,
            }
        };
        self.tmp = None;
        if timed_out {
            app.message = Some("the preview timed out and was stopped".into());
        } else if code != 0 {
            app.message = Some(format!("the dry run failed (exit {code})"));
        }
        app.set_review(lines);
    }

    fn install(&mut self, app: &mut App) {
        if self.child.is_some() {
            return;
        }
        let Some(device) = app.device.clone() else {
            return;
        };
        let started = self.write_config(app).and_then(|cfg| {
            Child::spawn(
                &self.exe,
                &install_args(&device, &cfg, false),
                &Self::env(app),
            )
        });
        match started {
            Ok(mut c) => {
                // the CLI asks for the path again; the user already typed it in the TUI
                if let Err(e) = c.send_line(&device) {
                    app.on_event(ChildEvent::Line(e));
                }
                self.child = Some(c);
            }
            Err(e) => {
                self.tmp = None;
                app.on_event(ChildEvent::Line(e));
                app.on_event(ChildEvent::Exited(127));
            }
        }
    }

    /// Forwards the child's output to the app; cleans up when it exits.
    pub fn poll(&mut self, app: &mut App) {
        let mut events = Vec::new();
        let mut gone = false;
        if let Some(c) = &self.child {
            loop {
                match c.rx.try_recv() {
                    Ok(ev) => events.push(ev),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        gone = true;
                        break;
                    }
                }
            }
        }
        for ev in events {
            let exited = matches!(ev, ChildEvent::Exited(_));
            app.on_event(ev);
            if exited {
                self.child = None;
                self.tmp = None;
                return;
            }
        }
        if gone {
            app.on_event(ChildEvent::Exited(127));
            self.child = None;
            self.tmp = None;
        }
    }
}

impl Drop for Runtime {
    /// If the TUI itself ends (an error, a panic) while an install runs, stop it cleanly and wait
    /// for its cleanup instead of leaving a half-finished install with nobody watching.
    fn drop(&mut self) {
        if let Some(c) = self.child.take() {
            eprintln!("stopping the install, please wait...");
            c.interrupt();
            let end = Instant::now() + Duration::from_secs(60);
            while let Ok(ev) =
                c.rx.recv_timeout(end.saturating_duration_since(Instant::now()))
            {
                if matches!(ev, ChildEvent::Exited(_)) {
                    break;
                }
            }
        }
    }
}

fn is_root() -> bool {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").is_ok_and(|m| m.uid() == 0)
}

/// Opens the TUI. Refuses without a terminal; always restores the terminal on the way out
/// (ratatui's `init` also installs a panic hook that restores it).
pub fn run(initial: Option<Config>) -> Result<(), String> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err("arch-to-go tui needs a terminal".into());
    }
    let exe = std::env::current_exe().map_err(|e| format!("cannot find myself: {e}"))?;
    let mut app = App::new(initial, is_root(), Vec::new());
    match list_usb_disks() {
        Ok(d) => app.disks = d,
        Err(e) => app.message = Some(e),
    }
    let mut rt = Runtime::new(exe);
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app, &mut rt);
    ratatui::restore();
    result
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    rt: &mut Runtime,
) -> Result<(), String> {
    loop {
        rt.poll(app);
        terminal.draw(|f| draw(app, f)).map_err(|e| e.to_string())?;
        if !event::poll(Duration::from_millis(100)).map_err(|e| e.to_string())? {
            continue;
        }
        if let Event::Key(k) = event::read().map_err(|e| e.to_string())? {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            let effect = app.handle_key(k);
            if effect == Effect::Quit {
                return Ok(());
            }
            rt.perform(effect, app);
        }
    }
}
