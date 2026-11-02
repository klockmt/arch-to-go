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
