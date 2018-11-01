#[macro_use]
extern crate log;
extern crate failure;
extern crate simplelog;
extern crate structopt;
extern crate which;

mod error;
mod tool;

use error::*;
use failure::{Fail, ResultExt};
use simplelog::*;
use std::fs;
use std::path::PathBuf;
use std::process::{exit, Command};
use std::thread;
use std::time::Duration;
use structopt::StructOpt;
use tool::Tool;

static MKINITCPIO: &'static str = "MODULES=()
BINARIES=()
FILES=()
HOOKS=(base udev block filesystems keyboard fsck)";

#[derive(Fail, Debug)]
#[fail(display = "Process failed")]
pub struct ProcessFailed;

trait CommandExt {
    fn run(&mut self, context: ErrorKind) -> Result<(), Error>;
}

impl CommandExt for Command {
    fn run(&mut self, context: ErrorKind) -> Result<(), Error> {
        let exit_status = self.spawn().context(context)?.wait().context(context)?;

        if !exit_status.success() {
            return Err(ProcessFailed {}.context(context).into());
        }

        Ok(())
    }
}

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
    let partprobe = Tool::find("partprobe")?;
    let pacstrap = Tool::find("pacstrap")?;
    let arch_chroot = Tool::find("arch-chroot")?;
    let mount = Tool::find("mount")?;
    let umount = Tool::find("umount")?;
    let mkfat = Tool::find("mkfs.fat")?;
    let mkbtrfs = Tool::find("mkfs.btrfs")?;

    if !(disk.starts_with("/dev/disk/by-id")
        && (disk
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|ref f| f.starts_with("usb-"))
            .is_some()))
    {
        return Err(ErrorKind::NotUSB.into());
    }

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
        .run(ErrorKind::Creation)?;
    partprobe.execute().run(ErrorKind::Creation)?;

    thread::sleep(Duration::from_millis(1000));

    info!("Formatting filesystems");
    mkfat
        .execute()
        .arg("-F32")
        .arg(format!("{}-part2", disk.display()))
        .run(ErrorKind::Creation)?;
    mkbtrfs
        .execute()
        .arg("-f")
        .arg(format!("{}-part3", disk.display()))
        .run(ErrorKind::Creation)?;

    info!("Mounting filesystems to /mnt/dok");
    mount
        .execute()
        .arg(format!("{}-part3", disk.display()))
        .arg("/mnt/dok")
        .run(ErrorKind::Creation)?;

    fs::create_dir("/mnt/dok/boot").context(ErrorKind::Creation)?;

    mount
        .execute()
        .arg(format!("{}-part2", disk.display()))
        .arg("/mnt/dok/boot")
        .run(ErrorKind::Creation)?;

    info!("Bootstrapping system");
    pacstrap
        .execute()
        .args(&[
            "-c",
            "/mnt/dok",
            "base",
            "grub",
            "efibootmgr",
            "intel-ucode",
            "networkmanager",
            "btrfs-progs",
        ]).run(ErrorKind::Creation)?;

    arch_chroot
        .execute()
        .args(&["/mnt/dok", "systemctl", "enable", "NetworkManager"])
        .run(ErrorKind::Creation)?;

    info!("Generating initramfs");
    fs::write("/mnt/dok/etc/mkinitcpio.conf", MKINITCPIO).context(ErrorKind::Creation)?;
    arch_chroot
        .execute()
        .args(&["/mnt/dok", "mkinitcpio", "-p", "linux"])
        .run(ErrorKind::Creation)?;

    info!("Installing the Bootloader");
    arch_chroot
        .execute()
        .args(&["/mnt/dok", "bash", "-c"])
        .arg(format!("grub-install --target=i386-pc --boot-directory /boot {} && grub-install --target=x86_64-efi --efi-directory /boot --boot-directory /boot --removable &&  grub-mkconfig -o /boot/grub/grub.cfg", disk.display()))
        .run(ErrorKind::Creation)?;

    info!("Unmounting filesystems");
    umount
        .execute()
        .args(&["/mnt/dok/boot", "/mnt/dok"])
        .run(ErrorKind::Creation)?;

    sync.execute().run(ErrorKind::Creation)?;

    Ok(())
}

fn main() {
    let app = App::from_args();

    CombinedLogger::init(vec![
        TermLogger::new(LevelFilter::Debug, Config::default()).unwrap(),
    ]).unwrap();

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
