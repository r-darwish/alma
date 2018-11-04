#[macro_use]
extern crate log;
extern crate failure;
extern crate nix;
extern crate simplelog;
extern crate structopt;
extern crate tempfile;
extern crate which;
use nix::sys::signal;

mod error;
mod mountstack;
mod process;
mod tool;

use error::*;
use failure::{Fail, ResultExt};
use mountstack::{Filesystem, MountStack};
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

#[derive(StructOpt)]
#[structopt(name = "alma", about = "Arch Linux Mobile Applicance")]
enum App {
    #[structopt(name = "create", about = "Create a new Arch Linux USB")]
    Create {
        #[structopt(parse(from_os_str))]
        disk: PathBuf,
    },
}

fn create(disk: PathBuf) -> Result<(), Error> {
    let sgdisk = Tool::find("sgdisk")?;
    let sync = Tool::find("sync")?;
    let pacstrap = Tool::find("pacstrap")?;
    let arch_chroot = Tool::find("arch-chroot")?;
    let genfstab = Tool::find("genfstab")?;
    let mkfat = Tool::find("mkfs.fat")?;
    let mkbtrfs = Tool::find("mkfs.btrfs")?;
    let mut mount_stack = MountStack::new();

    if !(disk.starts_with("/dev/disk/by-id")
        && (disk
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|ref f| f.starts_with("usb-"))
            .is_some()))
    {
        return Err(ErrorKind::NotUSB.into());
    }

    let mount_point = tempdir().context(ErrorKind::TmpDirError)?;

    info!("Partitioning the disk");
    sgdisk
        .execute()
        .args(&[
            "-Z",
            "-o",
            "--new=1::+10M",
            "--new=2::+500M",
            "--largest-new=3",
            "--typecode=1:EF02",
            "--typecode=2:EF00",
        ]).arg(&disk)
        .run(ErrorKind::Partitioning)?;

    thread::sleep(Duration::from_millis(1000));

    info!("Formatting filesystems");
    mkfat
        .execute()
        .arg("-F32")
        .arg(format!("{}-part2", disk.display()))
        .run(ErrorKind::Formatting)?;
    mkbtrfs
        .execute()
        .arg("-f")
        .arg(format!("{}-part3", disk.display()))
        .run(ErrorKind::Formatting)?;

    info!("Mounting filesystems to {}", mount_point.path().display());
    mount_stack
        .mount(
            &PathBuf::from(format!("{}-part3", disk.display())),
            &mount_point.path(),
            Filesystem::Btrfs,
            Some("compress=zstd"),
        ).context(ErrorKind::Mounting)?;

    let boot_point = mount_point.path().join("boot");
    fs::create_dir(&boot_point).context(ErrorKind::CreateBoot)?;

    mount_stack
        .mount(
            &PathBuf::from(format!("{}-part2", disk.display())),
            &boot_point,
            Filesystem::Vfat,
            None,
        ).context(ErrorKind::Mounting)?;

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
        ]).run(ErrorKind::Pacstrap)?;

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
        .arg(format!("grub-install --target=i386-pc --boot-directory /boot {} && grub-install --target=x86_64-efi --efi-directory /boot --boot-directory /boot --removable &&  grub-mkconfig -o /boot/grub/grub.cfg", disk.display()))
        .run(ErrorKind::Bootloader)?;

    info!("Unmounting filesystems");
    drop(mount_stack);
    sync.execute().run(ErrorKind::Sync)?;

    Ok(())
}

extern "C" fn handle_sigint(_: i32) {
    warn!("Interrupted");
}

fn main() {
    let app = App::from_args();

    CombinedLogger::init(vec![
        TermLogger::new(LevelFilter::Debug, Config::default()).unwrap(),
    ]).unwrap();

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

    let result = match app {
        App::Create { disk } => create(disk),
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
