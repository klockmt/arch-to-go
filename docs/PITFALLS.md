# Pitfalls

What broke, why, and the test that keeps it from breaking again.

| What broke | Why | Guarded by |
|---|---|---|
| `ln -sf ... /etc/resolv.conf` inside `arch-chroot` fails: Device or resource busy | arch-chroot bind-mounts the host's resolv.conf there | `plan_profile::iwd_networkd_writes_both_configs_and_resolv_symlink` |
| Target used the host's mirrorlist (CachyOS mirror, 404s) | pacstrap copies the host's `/etc/pacman.d/mirrorlist` | `plan_base::target_mirrorlist_is_official_and_written_after_pacstrap` |
| A USB disk that is a member of the host's multi-device btrfs root passed the safety checks | findmnt names only one device per mounted filesystem | `safety::rejects_disk_sharing_a_filesystem_with_the_host` |
| The target could change between the safety check and the wipe (long prompt or paru build) | the check ran once, early | `cli::disk_is_checked_again_right_before_the_wipe` |
| The admin password was readable by the downloaded AUR build | only one variable was stripped, and not for the paru build | `runner::child_env_keeps_only_an_allow_list` |
| paru built on the host used `-march=native` and CachyOS's libalpm | host makepkg.conf; not portable | `paru::paru_comes_from_the_prebuilt_package_not_a_host_build` |
| "installed" printed with writes unflushed or only a lazy unmount | no sync; unmount errors dropped | `plan_all::plan_ends_with_sync`, `runner::finish_*` |
