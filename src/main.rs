mod alma;
mod block;
mod cryptsetup;
mod error;
mod mountstack;
mod process;
mod tool;

use crate::alma::ALMA;
use crate::cryptsetup::EncryptedDevice;
use crate::error::*;
use crate::process::CommandExt;
use crate::tool::Tool;
use failure::{Fail, ResultExt};
use log::{debug, error, info, warn};
use nix::sys::signal;
use simplelog::*;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::thread;
use std::time::Duration;
use structopt::StructOpt;
use tempfile::tempdir;

static MKINITCPIO: &'static str = "MODULES=()
BINARIES=()
FILES=()
HOOKS=(base udev keyboard consolefont block encrypt filesystems keyboard fsck)";

static JOURNALD_CONF: &'static str = "
[Journal]
Storage=volatile
SystemMaxUse=16M
";

#[derive(StructOpt)]
#[structopt(name = "alma", about = "Arch Linux Mobile Appliance")]
struct App {
    /// Verbose output
    #[structopt(short = "v", long = "verbose")]
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
    #[structopt(parse(from_os_str))]
    block_device: PathBuf,

    /// Additional pacakges to install
    #[structopt(short = "p", long = "extra-packages", value_name = "package")]
    extra_packages: Vec<String>,

    /// Enter interactive chroot before unmounting the drive
    #[structopt(short = "i", long = "interactive")]
    interactive: bool,

    /// Encrypt the root partition
    #[structopt(short = "e", long = "encrypted-root")]
    encrypted_root: bool,
}

#[derive(StructOpt)]
struct ChrootCommand {
    /// Path starting with /dev/disk/by-id for the USB drive
    #[structopt(parse(from_os_str))]
    block_device: PathBuf,

    /// Open an encrypted root partition
    #[structopt(short = "e", long = "encrypted-root")]
    encrypted_root: bool,

    /// Optional command to run
    #[structopt()]
    command: Vec<String>,
}

fn fix_fstab(fstab: &str) -> String {
    fstab
        .lines()
        .filter(|line| !line.contains("swap") && !line.starts_with('#'))
        .collect::<Vec<&str>>()
        .join("\n")
}

fn create(command: CreateCommand) -> Result<(), Error> {
    let sgdisk = Tool::find("sgdisk")?;
    let pacstrap = Tool::find("pacstrap")?;
    let arch_chroot = Tool::find("arch-chroot")?;
    let genfstab = Tool::find("genfstab")?;
    let mkfat = Tool::find("mkfs.fat")?;
    let mkext4 = Tool::find("mkfs.ext4")?;
    let cryptsetup = if command.encrypted_root {
        Some(Tool::find("cryptsetup")?)
    } else {
        None
    };
    let blkid = if command.encrypted_root {
        Some(Tool::find("blkid")?)
    } else {
        None
    };

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
            "--new=1::+100M",
            "--new=2::+1M",
            "--largest-new=3",
            "--typecode=1:EF00",
            "--typecode=2:EF02",
        ])
        .arg(&disk_path)
        .run(ErrorKind::Partitioning)?;

    thread::sleep(Duration::from_millis(1000));

    info!("Formatting filesystems");
    let boot_partition = block_device.partition_device_path(1)?;
    mkfat
        .execute()
        .arg("-F32")
        .arg(&boot_partition)
        .run(ErrorKind::Formatting)?;

    let root_partition = block_device.partition_device_path(3)?;
    let encrypted_root = if let Some(cryptsetup) = &cryptsetup {
        info!("Encrypting the root filesystem");
        EncryptedDevice::prepare(&cryptsetup, &root_partition)?;
        Some(EncryptedDevice::open(
            cryptsetup,
            &root_partition,
            "alma_root",
        )?)
    } else {
        None
    };

    mkext4
        .execute()
        .arg("-F")
        .arg(if let Some(device) = &encrypted_root {
            device.path()
        } else {
            &root_partition
        })
        .run(ErrorKind::Formatting)?;

    let alma = ALMA::new(block_device, encrypted_root);
    let mount_stack = alma.mount(mount_point.path())?;

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
            "broadcom-wl",
        ])
        .args(&command.extra_packages)
        .run(ErrorKind::Pacstrap)?;

    let fstab = fix_fstab(
        &genfstab
            .execute()
            .arg("-U")
            .arg(mount_point.path())
            .run_text_output(ErrorKind::Fstab)?,
    );
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
    )
    .context(ErrorKind::PostInstallation)?;

    info!("Setting locale");
    fs::OpenOptions::new()
        .append(true)
        .write(true)
        .open(mount_point.path().join("etc/locale.gen"))
        .and_then(|mut locale_gen| locale_gen.write_all(b"en_US.UTF-8 UTF-8\n"))
        .context(ErrorKind::Locale)?;
    fs::write(
        mount_point.path().join("etc/locale.conf"),
        "LANG=en_US.UTF-8",
    )
    .context(ErrorKind::Locale)?;
    arch_chroot
        .execute()
        .arg(mount_point.path())
        .arg("locale-gen")
        .run(ErrorKind::Locale)?;

    info!("Generating initramfs");
    fs::write(mount_point.path().join("etc/mkinitcpio.conf"), MKINITCPIO)
        .context(ErrorKind::Initramfs)?;
    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&["mkinitcpio", "-p", "linux"])
        .run(ErrorKind::Initramfs)?;

    if cryptsetup.is_some() {
        debug!("Setting up GRUB for an encrypted root partition");

        let uuid = blkid
            .unwrap()
            .execute()
            .arg(root_partition)
            .args(&["-o", "value", "-s", "UUID"])
            .run_text_output(ErrorKind::Partitioning)?;
        let trimmed = uuid.trim();
        debug!("Root partition UUID: {}", trimmed);

        let mut grub_file = fs::OpenOptions::new()
            .append(true)
            .open(mount_point.path().join("etc/default/grub"))
            .context(ErrorKind::Bootloader)?;

        write!(
            &mut grub_file,
            "GRUB_CMDLINE_LINUX=\"cryptdevice=UUID={}:luks_root\"",
            trimmed
        )
        .context(ErrorKind::Bootloader)?;
    }

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

    Ok(())
}

fn chroot(command: ChrootCommand) -> Result<(), Error> {
    let arch_chroot = Tool::find("arch-chroot")?;
    let cryptsetup = if command.encrypted_root {
        Some(Tool::find("cryptsetup")?)
    } else {
        None
    };

    let block_device = block::BlockDevice::from_path(command.block_device)?;

    let mount_point = tempdir().context(ErrorKind::TmpDirError)?;
    let root_partition = block_device.partition_device_path(3)?;
    let encrypted_root = if let Some(cryptsetup) = &cryptsetup {
        Some(EncryptedDevice::open(
            cryptsetup,
            &root_partition,
            "alma_root",
        )?)
    } else {
        None
    };

    let alma = ALMA::new(block_device, encrypted_root);
    let mount_stack = alma.mount(mount_point.path())?;

    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&command.command)
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
                error!("Caused by: {}", cause);
            }
            exit(1);
        }
    }
}
