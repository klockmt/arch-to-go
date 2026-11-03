//! Runs the CLI as a child process and turns its output into `ChildEvent`s.

use crate::app::ChildEvent;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::{Duration, Instant};

/// Arguments of `arch-to-go install` for the given device and config file.
pub fn install_args(device: &str, config: &Path, dry_run: bool) -> Vec<String> {
    let mut a = vec![
        "install".to_owned(),
        device.to_owned(),
        "--config".to_owned(),
        config.display().to_string(),
    ];
    if dry_run {
        a.push("--dry-run".into());
    }
    a
}

pub struct Child {
    pub rx: Receiver<ChildEvent>,
    pid: u32,
    stdin: Option<std::process::ChildStdin>,
}

/// Sends every line (a trailing partial one too) until EOF. Reads bytes and converts lossily:
/// a stalled reader would block the child on a full pipe.
fn pump(r: impl Read, tx: Sender<ChildEvent>) {
    let mut r = BufReader::new(r);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match r.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => return,
            Ok(_) => {
                while matches!(buf.last(), Some(b'\n' | b'\r')) {
                    buf.pop();
                }
                let line = String::from_utf8_lossy(&buf).into_owned();
                if tx.send(ChildEvent::Line(line)).is_err() {
                    return;
                }
            }
        }
    }
}

impl Child {
    /// Starts `exe` in its own process group, so an interrupt reaches the whole install.
    pub fn spawn(exe: &Path, args: &[String], env: &[(String, String)]) -> Result<Child, String> {
        let mut child = {
            let mut tries = 0;
            loop {
                let mut cmd = Command::new(exe);
                cmd.args(args)
                    .envs(env.iter().map(|(k, v)| (k, v)))
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .process_group(0);
                match cmd.spawn() {
                    // another thread's fork can hold a just-written script open for a moment
                    Err(e) if e.kind() == ErrorKind::ExecutableFileBusy && tries < 50 => {
                        tries += 1;
                        thread::sleep(Duration::from_millis(20));
                    }
                    other => {
                        break other.map_err(|e| format!("cannot start {}: {e}", exe.display()))?;
                    }
                }
            }
        };
        let pid = child.id();
        let stdin = child.stdin.take();
        let (tx, rx) = channel();
        let out = child.stdout.take().expect("piped stdout");
        let err = child.stderr.take().expect("piped stderr");
        let (t1, t2) = (tx.clone(), tx.clone());
        let (done_tx, done_rx) = channel::<()>();
        let (d1, d2) = (done_tx.clone(), done_tx);
        thread::spawn(move || {
            pump(out, t1);
            let _ = d1.send(());
        });
        thread::spawn(move || {
            pump(err, t2);
            let _ = d2.send(());
        });
        thread::spawn(move || {
            let code = match child.wait() {
                Ok(s) => s.code().unwrap_or_else(|| 128 + s.signal().unwrap_or(0)),
                Err(_) => 127,
            };
            // The output already in the pipes is sent first, but a process an install step left
            // running (a daemon) can keep the pipes open for ever: wait for the readers only briefly.
            let deadline = Instant::now() + Duration::from_secs(2);
            for _ in 0..2 {
                let left = deadline.saturating_duration_since(Instant::now());
                if done_rx.recv_timeout(left).is_err() {
                    break;
                }
            }
            let _ = tx.send(ChildEvent::Exited(code));
        });
        Ok(Child { rx, pid, stdin })
    }

    pub fn send_line(&mut self, line: &str) -> Result<(), String> {
        let stdin = self.stdin.as_mut().ok_or("the child's input is closed")?;
        writeln!(stdin, "{line}")
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("cannot write to the child: {e}"))
    }

    /// Sends SIGINT to the child's whole process group, as Ctrl-C in a terminal would.
    pub fn interrupt(&self) {
        let _ = Command::new("kill")
            .args(["-INT", "--", &format!("-{}", self.pid)])
            .status();
    }
}
