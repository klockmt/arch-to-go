//! Unmounts `/mnt` when dropped, so a failed or interrupted run leaves no mount behind.

use crate::runner::Runner;
use crate::step::Cmd;
use std::cell::RefCell;
use std::rc::Rc;

/// How the unmount went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unmount {
    Clean,
    /// Only the lazy unmount worked: something still held `/mnt`.
    Lazy,
    Failed,
}

pub struct MountGuard {
    runner: Rc<RefCell<dyn Runner>>,
    done: bool,
}

impl MountGuard {
    /// Build it only for real runs: a dry run mounts nothing.
    pub fn new(runner: Rc<RefCell<dyn Runner>>) -> Self {
        MountGuard {
            runner,
            done: false,
        }
    }

    /// Unmounts now and says how it went; `drop` then does nothing more.
    pub fn finish(mut self) -> Unmount {
        self.unmount()
    }

    fn unmount(&mut self) -> Unmount {
        self.done = true;
        let mut r = self.runner.borrow_mut();
        if r.run_cleanup(&Cmd::new(&["umount", "-R", "/mnt"])).is_ok() {
            return Unmount::Clean;
        }
        // lazy as a second try: chroot processes (gpg-agent) can hold /mnt open
        match r.run_cleanup(&Cmd::new(&["umount", "-R", "-l", "/mnt"])) {
            Ok(()) => Unmount::Lazy,
            Err(_) => Unmount::Failed,
        }
    }
}

impl Drop for MountGuard {
    fn drop(&mut self) {
        if !self.done {
            self.unmount();
        }
    }
}
