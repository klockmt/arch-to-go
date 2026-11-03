use arch_to_go_core::config::Config;
use arch_to_go_core::plan::{Ctx, build};
use arch_to_go_core::step::Step;

#[test]
fn plan_ends_with_sync() {
    // btrfs commits about every 30 s: without a final sync, unplugging right after the
    // run can lose the last steps (passwords, enabled services)
    let cfg = Config::from_toml("[users]\nadmin = \"tester\"\n").unwrap();
    let ctx = Ctx::new("/dev/sda", "11111111-2222-3333-4444-555555555555", "pw").unwrap();
    let steps = build(&cfg, &ctx).unwrap();
    match steps.last() {
        Some(Step::Run { cmd, .. }) => assert_eq!(cmd.argv, ["sync"]),
        other => panic!("last step is {other:?}"),
    }
}

#[test]
fn gpg_agent_is_stopped_before_the_final_sync() {
    // pacstrap and the chroot pacman calls start a gpg-agent on the target's keyring;
    // left running it holds /mnt, so the unmount can only be lazy and a loop device stays attached
    let cfg = Config::from_toml("[users]\nadmin = \"tester\"\n").unwrap();
    let ctx = Ctx::new("/dev/sda", "11111111-2222-3333-4444-555555555555", "pw").unwrap();
    let steps = build(&cfg, &ctx).unwrap();
    let argv = |s: &Step| match s {
        Step::Run { cmd, .. } => cmd.argv.clone(),
        _ => vec![],
    };
    let n = steps.len();
    assert_eq!(
        argv(&steps[n - 2]),
        [
            "gpgconf",
            "--homedir",
            "/mnt/etc/pacman.d/gnupg",
            "--kill",
            "all"
        ]
    );
    assert_eq!(argv(&steps[n - 1]), ["sync"]);
}
