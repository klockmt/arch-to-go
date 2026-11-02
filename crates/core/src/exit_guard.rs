//! Unmounts `/mnt` when dropped, so a failed or interrupted run leaves no mount behind.

use crate::runner::Runner;
use crate::step::Cmd;
use std::cell::RefCell;
use std::rc::Rc;

pub struct MountGuard {
    runner: Rc<RefCell<dyn Runner>>,
}

impl MountGuard {
    /// Build it only for real runs: a dry run mounts nothing.
    pub fn new(runner: Rc<RefCell<dyn Runner>>) -> Self {
        MountGuard { runner }
    }
}

impl Drop for MountGuard {
    fn drop(&mut self) {
        let mut r = self.runner.borrow_mut();
        // lazy as a second try: chroot processes (gpg-agent) can hold /mnt open
        if r.run(&Cmd::new(&["umount", "-R", "/mnt"])).is_err() {
            let _ = r.run(&Cmd::new(&["umount", "-R", "-l", "/mnt"]));
        }
    }
}
