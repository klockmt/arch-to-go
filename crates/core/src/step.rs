//! The unit of an install plan: a command or a file write, plus a description.

use std::fmt;

/// A value that must never be printed or logged (a password).
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Secret(s.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

/// A program and its arguments, run without a shell. `stdin` carries secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    pub argv: Vec<String>,
    pub stdin: Option<Secret>,
}

impl Cmd {
    pub fn new<S: AsRef<str>>(argv: &[S]) -> Self {
        Cmd {
            argv: argv.iter().map(|s| s.as_ref().to_owned()).collect(),
            stdin: None,
        }
    }

    pub fn with_stdin(mut self, s: Secret) -> Self {
        self.stdin = Some(s);
        self
    }

    /// The command line for logs, with stdin shown as `***`.
    pub fn display(&self) -> String {
        let line = self.argv.join(" ");
        match self.stdin {
            Some(_) => format!("{line} <<< ***"),
            None => line,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Run {
        desc: String,
        cmd: Cmd,
    },
    WriteFile {
        desc: String,
        path: String,
        contents: String,
        mode: u32,
    },
}

impl Step {
    pub fn desc(&self) -> &str {
        match self {
            Step::Run { desc, .. } | Step::WriteFile { desc, .. } => desc,
        }
    }
}
