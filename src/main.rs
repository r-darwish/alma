extern crate failure;
extern crate log;
extern crate nix;
extern crate simplelog;
extern crate structopt;
extern crate tempfile;
extern crate which;
use nix::sys::signal;

mod alma;
mod block;
mod error;
mod mountstack;
mod process;
mod tool;

use alma::ALMA;
use error::*;
use failure::{Fail, ResultExt};
use log::{debug, error, info, warn};
use process::CommandExt;
use simplelog::*;
use std::fs;
use std::path::PathBuf;
use std::process::exit;
use std::thread;
use std::time::Duration;
use structopt::StructOpt;
use tempfile::tempdir;
use tool::Tool;

static MKINITCPIO: &'static str = "MODULES=()
BINARIES=()
FILES=()
HOOKS=(base udev block filesystems keyboard fsck)";

static JOURNALD_CONF: &'static str = "
[Journal]
Storage=volatile
SystemMaxUse=16M
";

#[derive(StructOpt)]
#[structopt(name = "alma", about = "Arch Linux Mobile Appliance")]
struct App {
    #[structopt(short = "v", long = "verbose", help = "Verbose output")]
    verbose: bool,

    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(StructOpt)]
enum Command {
    #[structopt(name = "create", about = "Create a new Arch Linux USB")]
    Create(CreateCommand),

    #[structopt(name = "chroot", about = "Chroot into exiting Live USB")]
    Chroot(ChrootCommand),
}

#[derive(StructOpt)]
struct CreateCommand {
    /// Path starting with /dev/disk/by-id for the USB drive
    #[structopt(parse(from_os_str),)]
    block_device: PathBuf,

    /// Additional pacakges to install
    #[structopt(short = "p", long = "extra-packages", value_name = "package",)]
    extra_packages: Vec<String>,

    /// Enter interactive chroot before unmounting the drive
    #[structopt(short = "i", long = "interactive")]
    interactive: bool,
}

#[derive(StructOpt)]
struct ChrootCommand {
    /// Path starting with /dev/disk/by-id for the USB drive
    #[structopt(parse(from_os_str),)]
    block_device: PathBuf,
}

fn create(command: CreateCommand) -> Result<(), Error> {
    let sgdisk = Tool::find("sgdisk")?;
    let pacstrap = Tool::find("pacstrap")?;
    let arch_chroot = Tool::find("arch-chroot")?;
    let genfstab = Tool::find("genfstab")?;
    let mkfat = Tool::find("mkfs.fat")?;
    let mkbtrfs = Tool::find("mkfs.btrfs")?;

    let block_device = block::BlockDevice::from_path(command.block_device)?;

    let mount_point = tempdir().context(ErrorKind::TmpDirError)?;

    let disk_path = block_device.device_path();

    info!("Partitioning the block device");
    debug!("{:?}", disk_path);

    sgdisk
        .execute()
        .args(&[
            "-Z",
            "-o",
            "--new=1::+10M",
            "--new=2::+150M",
            "--largest-new=3",
            "--typecode=1:EF02",
            "--typecode=2:EF00",
        ]).arg(&disk_path)
        .run(ErrorKind::Partitioning)?;

    thread::sleep(Duration::from_millis(1000));

    info!("Formatting filesystems");
    let boot_partition = block_device.partition_device_path(2)?;
    mkfat
        .execute()
        .arg("-F32")
        .arg(&boot_partition)
        .run(ErrorKind::Formatting)?;

    let root_partition = block_device.partition_device_path(3)?;
    mkbtrfs
        .execute()
        .arg("-f")
        .arg(&root_partition)
        .run(ErrorKind::Formatting)?;

    let alma = ALMA::new(block_device);
    let mut mount_stack = alma.mount(mount_point.path())?;

    info!("Bootstrapping system");
    pacstrap
        .execute()
        .arg("-c")
        .arg(mount_point.path())
        .args(&[
            "base",
            "grub",
            "efibootmgr",
            "intel-ucode",
            "networkmanager",
            "btrfs-progs",
            "broadcom-wl",
        ]).args(&command.extra_packages)
        .run(ErrorKind::Pacstrap)?;

    let fstab = genfstab
        .execute()
        .arg("-U")
        .arg(mount_point.path())
        .run_text_output(ErrorKind::Fstab)?
        .replace("relatime", "noatime");
    debug!("fstab:\n{}", fstab);
    fs::write(mount_point.path().join("etc/fstab"), fstab).context(ErrorKind::Fstab)?;

    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&["systemctl", "enable", "NetworkManager"])
        .run(ErrorKind::PostInstallation)?;

    info!("Configuring journald");
    fs::write(
        mount_point.path().join("etc/systemd/journald.conf"),
        JOURNALD_CONF,
    ).context(ErrorKind::PostInstallation)?;

    info!("Generating initramfs");
    fs::write(mount_point.path().join("etc/mkinitcpio.conf"), MKINITCPIO)
        .context(ErrorKind::Initramfs)?;
    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&["mkinitcpio", "-p", "linux"])
        .run(ErrorKind::Initramfs)?;

    info!("Installing the Bootloader");
    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&["bash", "-c"])
        .arg(format!("grub-install --target=i386-pc --boot-directory /boot {} && grub-install --target=x86_64-efi --efi-directory /boot --boot-directory /boot --removable &&  grub-mkconfig -o /boot/grub/grub.cfg", disk_path.display()))
        .run(ErrorKind::Bootloader)?;

    if command.interactive {
        info!("Dropping you to chroot. Do as you wish to customize the installation");
        arch_chroot
            .execute()
            .arg(mount_point.path())
            .run(ErrorKind::Interactive)?;
    }

    info!("Unmounting filesystems");
    mount_stack.umount()?;

    info!("Installation succeeded. It is now safe to unplug your device.");

    Ok(())
}

fn chroot(command: ChrootCommand) -> Result<(), Error> {
    let arch_chroot = Tool::find("arch-chroot")?;
    let block_device = block::BlockDevice::from_path(command.block_device)?;

    let mount_point = tempdir().context(ErrorKind::TmpDirError)?;
    let alma = ALMA::new(block_device);
    let mut mount_stack = alma.mount(mount_point.path())?;

    arch_chroot
        .execute()
        .arg(mount_point.path())
        .run(ErrorKind::Interactive)?;

    info!("Unmounting filesystems");
    mount_stack.umount()?;

    Ok(())
}

extern "C" fn handle_sigint(_: i32) {
    warn!("Interrupted");
}

fn main() {
    let app = App::from_args();

    let log_level = if app.verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    CombinedLogger::init(vec![TermLogger::new(log_level, Config::default()).unwrap()]).unwrap();

    let sig_action = signal::SigAction::new(
        signal::SigHandler::Handler(handle_sigint),
        signal::SaFlags::empty(),
        signal::SigSet::empty(),
    );
    unsafe {
        signal::sigaction(signal::SIGINT, &sig_action).unwrap();
        signal::sigaction(signal::SIGTERM, &sig_action).unwrap();
        signal::sigaction(signal::SIGQUIT, &sig_action).unwrap();
    }

    let result = match app.cmd {
        Command::Create(command) => create(command),
        Command::Chroot(command) => chroot(command),
    };

    match result {
        Ok(()) => {
            exit(0);
        }
        Err(error) => {
            error!("{}", error);
            if let Some(cause) = error.cause() {
                error!("  {}", cause);
            }
            exit(1);
        }
    }
}
