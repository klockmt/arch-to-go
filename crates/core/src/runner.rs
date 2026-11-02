//! Runs plan steps. `DryRunner` prints, `MockRunner` records, `RealRunner` executes.

use crate::step::{Cmd, Step};
use std::io::Write;

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("empty command")]
    Empty,
    #[error("cannot start `{cmd}`: {source}")]
    Spawn { cmd: String, source: std::io::Error },
    #[error("`{cmd}` failed with {status}")]
    Failed { cmd: String, status: String },
    #[error("i/o error on {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("injected failure at call {0}")]
    Injected(usize),
}

pub trait Runner {
    fn run(&mut self, cmd: &Cmd) -> Result<(), RunError>;
    fn write_file(&mut self, path: &str, contents: &str, mode: u32) -> Result<(), RunError>;
    /// Called before each step with its description.
    fn announce(&mut self, _desc: &str) {}
}

/// Runs the steps in order and stops at the first failure.
pub fn execute(steps: &[Step], r: &mut dyn Runner) -> Result<(), RunError> {
    for step in steps {
        r.announce(step.desc());
        match step {
            Step::Run { cmd, .. } => r.run(cmd)?,
            Step::WriteFile {
                path,
                contents,
                mode,
                ..
            } => r.write_file(path, contents, *mode)?,
        }
    }
    Ok(())
}

pub struct DryRunner<W: Write> {
    out: W,
}

impl<W: Write> DryRunner<W> {
    pub fn new(out: W) -> Self {
        DryRunner { out }
    }
}

impl<W: Write> Runner for DryRunner<W> {
    fn run(&mut self, cmd: &Cmd) -> Result<(), RunError> {
        writeln!(self.out, "[dry-run] {}", cmd.display()).map_err(|e| RunError::Io {
            path: "stdout".into(),
            source: e,
        })
    }

    fn write_file(&mut self, path: &str, _contents: &str, mode: u32) -> Result<(), RunError> {
        writeln!(self.out, "[dry-run] write {path} (mode {mode:o})").map_err(|e| RunError::Io {
            path: "stdout".into(),
            source: e,
        })
    }

    fn announce(&mut self, desc: &str) {
        let _ = writeln!(self.out, "==> {desc}");
    }
}

/// Records every call. `fail_at` makes the call with that 0-based index fail
/// (the failing call is still recorded).
#[derive(Debug, Default)]
pub struct MockRunner {
    pub cmds: Vec<Cmd>,
    pub files: Vec<(String, String)>,
    pub fail_at: Option<usize>,
    pub calls: usize,
}

impl MockRunner {
    fn tick(&mut self) -> Result<(), RunError> {
        let n = self.calls;
        self.calls += 1;
        if self.fail_at == Some(n) {
            return Err(RunError::Injected(n));
        }
        Ok(())
    }
}

impl Runner for MockRunner {
    fn run(&mut self, cmd: &Cmd) -> Result<(), RunError> {
        self.cmds.push(cmd.clone());
        self.tick()
    }

    fn write_file(&mut self, path: &str, contents: &str, _mode: u32) -> Result<(), RunError> {
        self.files.push((path.into(), contents.into()));
        self.tick()
    }
}

pub struct RealRunner;

impl Runner for RealRunner {
    fn run(&mut self, cmd: &Cmd) -> Result<(), RunError> {
        use std::process::{Command, Stdio};
        let (prog, args) = cmd.argv.split_first().ok_or(RunError::Empty)?;
        let shown = cmd.argv.join(" ");
        let mut c = Command::new(prog);
        c.args(args);
        if cmd.stdin.is_some() {
            c.stdin(Stdio::piped());
        }
        let mut child = c.spawn().map_err(|e| RunError::Spawn {
            cmd: shown.clone(),
            source: e,
        })?;
        if let (Some(secret), Some(mut pipe)) = (&cmd.stdin, child.stdin.take()) {
            // the pipe closes when it drops, which ends the child's input
            pipe.write_all(secret.expose().as_bytes())
                .and_then(|_| pipe.write_all(b"\n"))
                .map_err(|e| RunError::Io {
                    path: shown.clone(),
                    source: e,
                })?;
        }
        let status = child.wait().map_err(|e| RunError::Spawn {
            cmd: shown.clone(),
            source: e,
        })?;
        if status.success() {
            Ok(())
        } else {
            Err(RunError::Failed {
                cmd: shown,
                status: status.to_string(),
            })
        }
    }

    fn write_file(&mut self, path: &str, contents: &str, mode: u32) -> Result<(), RunError> {
        use std::os::unix::fs::PermissionsExt;
        let io = |e| RunError::Io {
            path: path.into(),
            source: e,
        };
        if let Some(dir) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        std::fs::write(path, contents).map_err(io)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).map_err(io)
    }

    fn announce(&mut self, desc: &str) {
        eprintln!("==> {desc}");
    }
}
