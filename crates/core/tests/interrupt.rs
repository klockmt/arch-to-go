// Own test binary: INTERRUPTED is process-global and would race other tests.
use arch_to_go_core::runner::{INTERRUPTED, RealRunner, RunError, Runner};
use arch_to_go_core::step::Cmd;
use std::sync::atomic::Ordering;

#[test]
fn interrupt_flag_stops_the_next_command_and_file_write() {
    INTERRUPTED.store(true, Ordering::SeqCst);
    let e = RealRunner.run(&Cmd::new(&["true"])).unwrap_err();
    assert!(matches!(e, RunError::Interrupted), "{e}");
    let e = RealRunner
        .write_file("/nonexistent/never", "x", 0o644)
        .unwrap_err();
    assert!(matches!(e, RunError::Interrupted), "{e}");
    // cleanup (the unmount) must still run after Ctrl-C, or /mnt stays mounted
    RealRunner.run_cleanup(&Cmd::new(&["true"])).unwrap();
}
