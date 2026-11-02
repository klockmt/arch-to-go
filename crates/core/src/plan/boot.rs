//! Limine as a removable-path UEFI bootloader (`EFI/BOOT/BOOTX64.EFI`).
//! It never writes the host's NVRAM, so the drive boots on any machine.

use super::{Ctx, run, write};
use crate::config::{Config, Fs};
use crate::step::Step;

const HOOK: &str = include_str!("../../assets/limine.hook");

fn limine_conf(cfg: &Config, ctx: &Ctx) -> String {
    let flags = if cfg.disk.fs == Fs::Btrfs {
        " rootflags=subvol=@"
    } else {
        ""
    };
    let mut conf = String::from("timeout: 3\n");
    for k in &cfg.boot.kernels {
        let n = k.name();
        conf.push_str(&format!(
            "\n/Arch Linux ({n})\n    protocol: linux\n    path: boot():/vmlinuz-{n}\n    cmdline: root=PARTUUID={}{flags} rw\n",
            ctx.root_partuuid
        ));
        if cfg.portable.firmware {
            // microcode first, so it loads before the initramfs
            conf.push_str("    module_path: boot():/intel-ucode.img\n    module_path: boot():/amd-ucode.img\n");
        }
        conf.push_str(&format!("    module_path: boot():/initramfs-{n}.img\n"));
    }
    conf
}

pub fn boot_steps(cfg: &Config, ctx: &Ctx) -> Vec<Step> {
    vec![
        run(
            "install limine",
            &[
                "arch-chroot",
                "/mnt",
                "pacman",
                "-S",
                "--noconfirm",
                "--needed",
                "limine",
            ],
        ),
        run("create EFI/BOOT", &["mkdir", "-p", "/mnt/boot/EFI/BOOT"]),
        run(
            "copy BOOTX64.EFI to the removable path",
            &[
                "cp",
                "/mnt/usr/share/limine/BOOTX64.EFI",
                "/mnt/boot/EFI/BOOT/BOOTX64.EFI",
            ],
        ),
        write(
            "limine config",
            "/mnt/boot/limine.conf",
            &limine_conf(cfg, ctx),
            0o644,
        ),
        // recopy the EFI when the limine package updates, or boot keeps the old one
        write(
            "limine pacman hook",
            "/mnt/etc/pacman.d/hooks/limine.hook",
            HOOK,
            0o644,
        ),
    ]
}
