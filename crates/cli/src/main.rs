#[cfg(debug_assertions)]
mod fixture;
mod paru;
mod prompt;

use arch_to_go_core::config::Config;
use arch_to_go_core::exit_guard::{MountGuard, Unmount};
use arch_to_go_core::plan::{Ctx, build};
use arch_to_go_core::runner::{DryRunner, INTERRUPTED, RealRunner, execute};
use arch_to_go_core::safety::{HostSystem, SystemInfo, check_target, confirm_matches};
use clap::{Parser, Subcommand};
use std::cell::RefCell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::atomic::Ordering;

#[derive(Parser)]
#[command(
    name = "arch-to-go",
    about = "Install a portable Arch Linux on a USB disk"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install onto DEVICE (erases it). DRY_RUN=1 works like --dry-run.
    Install {
        device: String,
        #[arg(long)]
        config: Option<PathBuf>,
        /// Print the plan, run nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// Validate a config file
    CheckConfig { file: PathBuf },
    /// Open the terminal UI (editing needs no root; installing does)
    Tui {
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let result = match Cli::parse().cmd {
        Cmd::Install {
            device,
            config,
            dry_run,
        } => install(
            &device,
            config,
            dry_run || std::env::var("DRY_RUN").is_ok_and(|v| v == "1"),
        ),
        Cmd::CheckConfig { file } => load(&file).map(|_| println!("ok")),
        Cmd::Tui { config } => config
            .map(|f| load(&f))
            .transpose()
            .and_then(arch_to_go_tui::runtime::run),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn load(file: &PathBuf) -> Result<Config, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    Config::from_toml(&text).map_err(|e| e.to_string())
}

/// The fake machine is for tests: debug builds only.
fn system() -> (Box<dyn SystemInfo>, bool) {
    #[cfg(debug_assertions)]
    if let Ok(dir) = std::env::var("ARCH_TO_GO_SYSINFO_DIR") {
        return (
            Box::new(fixture::FixtureSystem(dir.into(), Default::default())),
            true,
        );
    }
    (Box::new(HostSystem), false)
}

fn is_root() -> bool {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").is_ok_and(|m| m.uid() == 0)
}

fn install(device: &str, config: Option<PathBuf>, dry: bool) -> Result<(), String> {
    let cfg = match config {
        Some(f) => load(&f)?,
        None => prompt::ask_config()?,
    };
    // the device argument always wins over disk.target in the file
    let (sys, fake) = system();
    if fake && !dry {
        return Err("ARCH_TO_GO_SYSINFO_DIR works only with --dry-run".into());
    }
    if !dry && !is_root() {
        return Err("a real install needs root (or use --dry-run)".into());
    }
    let allow_loop = std::env::var("ARCH_TO_GO_ALLOW_LOOP").is_ok_and(|v| v == "1");
    check_target(device, sys.as_ref(), allow_loop).map_err(|e| e.to_string())?;

    if !fake {
        let _ = std::process::Command::new("lsblk")
            .args(["-o", "NAME,SIZE,MODEL,FSTYPE,MOUNTPOINTS", device])
            .status();
    }
    if !dry {
        let typed = prompt::line(
            &format!("This ERASES {device}. Type the exact path to continue"),
            "",
        )?;
        confirm_matches(device, &typed).map_err(|e| e.to_string())?;
    }

    // asked before any write, so a typo cannot cost a half-installed disk
    let password = match std::env::var("ARCH_ADMIN_PASSWORD") {
        Ok(p) => p,
        Err(_) if dry => "dry-run".into(),
        Err(_) => prompt::password()?,
    };
    let partuuid = std::fs::read_to_string("/proc/sys/kernel/random/uuid")
        .map_err(|e| e.to_string())?
        .trim()
        .to_owned();
    let mut ctx = Ctx::new(device, &partuuid, &password).map_err(|e| e.to_string())?;
    if cfg.aur.paru {
        ctx = ctx.with_paru_pkg(if dry {
            "pkgs/paru-dry-run.pkg.tar.zst".to_owned()
        } else {
            paru::build()?
        });
    }
    let steps = build(&cfg, &ctx).map_err(|e| e.to_string())?;
    // the confirmation, the password prompt and a paru build can take minutes: the
    // name may now belong to another disk, so inspect it again right before the wipe
    check_target(device, sys.as_ref(), allow_loop).map_err(|e| e.to_string())?;

    if dry {
        return execute(&steps, &mut DryRunner::new(std::io::stdout())).map_err(|e| e.to_string());
    }
    ctrlc::set_handler(|| INTERRUPTED.store(true, Ordering::SeqCst)).map_err(|e| e.to_string())?;
    let runner = Rc::new(RefCell::new(RealRunner));
    let guard = MountGuard::new(runner.clone());
    let result = execute(&steps, &mut *runner.borrow_mut());
    let unmount = guard.finish();
    result.map_err(|e| e.to_string())?;
    match unmount {
        Unmount::Clean => eprintln!("installed on {device}"),
        Unmount::Lazy => eprintln!(
            "installed on {device}, but /mnt was only unmounted lazily (something still held it): \
             wait a few seconds and run `sync` before unplugging"
        ),
        Unmount::Failed => {
            return Err(format!(
                "installed on {device}, but /mnt could not be unmounted: do not unplug the drive yet"
            ));
        }
    }
    Ok(())
}
