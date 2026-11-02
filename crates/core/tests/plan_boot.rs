use arch_to_go_core::config::Config;
use arch_to_go_core::plan::Ctx;
use arch_to_go_core::plan::boot::boot_steps;
use arch_to_go_core::step::Step;

const PARTUUID: &str = "11111111-2222-3333-4444-555555555555";

fn steps(extra: &str) -> Vec<Step> {
    let cfg = Config::from_toml(&format!("[users]\nadmin = \"tester\"\n{extra}")).unwrap();
    boot_steps(&cfg, &Ctx::new("/dev/sda", PARTUUID, "pw").unwrap())
}

fn argvs(steps: &[Step]) -> Vec<String> {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Run { cmd, .. } => Some(cmd.argv.join(" ")),
            _ => None,
        })
        .collect()
}

fn file<'a>(steps: &'a [Step], path: &str) -> &'a str {
    steps
        .iter()
        .find_map(|s| match s {
            Step::WriteFile {
                path: p, contents, ..
            } if p == path => Some(contents.as_str()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no file written at {path}"))
}

#[test]
fn bootx64_copied_to_removable_path() {
    let a = argvs(&steps(""));
    let mkdir = a
        .iter()
        .position(|c| c == "mkdir -p /mnt/boot/EFI/BOOT")
        .expect("mkdir");
    let cp = a
        .iter()
        .position(|c| c == "cp /mnt/usr/share/limine/BOOTX64.EFI /mnt/boot/EFI/BOOT/BOOTX64.EFI")
        .expect("cp");
    assert!(mkdir < cp);
    assert!(a.contains(&"arch-chroot /mnt pacman -S --noconfirm --needed limine".to_string()));
}

#[test]
fn no_efibootmgr_or_nvram_write() {
    assert!(argvs(&steps("")).iter().all(|c| !c.contains("efibootmgr")
        && !c.contains("limine bios")
        && !c.contains("limine-install")));
}

#[test]
fn conf_has_entry_per_kernel_in_order() {
    let s = steps("[boot]\nkernels = [\"linux\", \"linux-lts\"]\n");
    let conf = file(&s, "/mnt/boot/limine.conf");
    let a = conf.find("/Arch Linux (linux)\n").expect("linux entry");
    let b = conf.find("/Arch Linux (linux-lts)\n").expect("lts entry");
    assert!(a < b, "first kernel is the default entry");
    assert!(conf.contains("path: boot():/vmlinuz-linux-lts\n"));
    assert!(conf.contains("module_path: boot():/initramfs-linux-lts.img\n"));
    assert!(conf.starts_with("timeout: 3\n"));
}

#[test]
fn conf_uses_partuuid_from_ctx() {
    let conf = file(&steps(""), "/mnt/boot/limine.conf").to_string();
    assert!(
        conf.contains(&format!(
            "cmdline: root=PARTUUID={PARTUUID} rootflags=subvol=@ rw\n"
        )),
        "{conf}"
    );
}

#[test]
fn rootflags_omitted_for_ext4() {
    let conf = file(&steps("[disk]\nfs = \"ext4\"\n"), "/mnt/boot/limine.conf").to_string();
    assert!(
        conf.contains(&format!("cmdline: root=PARTUUID={PARTUUID} rw\n")),
        "{conf}"
    );
    assert!(!conf.contains("rootflags"));
}

#[test]
fn ucode_is_loaded_before_the_initramfs_when_firmware_is_on() {
    let conf = file(&steps(""), "/mnt/boot/limine.conf").to_string();
    let intel = conf
        .find("module_path: boot():/intel-ucode.img")
        .expect("intel");
    let amd = conf
        .find("module_path: boot():/amd-ucode.img")
        .expect("amd");
    let img = conf
        .find("module_path: boot():/initramfs-linux.img")
        .expect("initramfs");
    assert!(intel < img && amd < img);
    let off = file(
        &steps("[portable]\nfirmware = false\n"),
        "/mnt/boot/limine.conf",
    )
    .to_string();
    assert!(!off.contains("ucode"));
}

#[test]
fn pacman_hook_installed() {
    let s = steps("");
    let hook = file(&s, "/mnt/etc/pacman.d/hooks/limine.hook");
    assert!(hook.contains("Target = limine"));
    assert!(hook.contains("cp /usr/share/limine/BOOTX64.EFI /boot/EFI/BOOT/BOOTX64.EFI"));
}
