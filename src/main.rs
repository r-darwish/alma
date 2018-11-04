#[macro_use]
extern crate log;
extern crate failure;
extern crate nix;
extern crate simplelog;
extern crate structopt;
extern crate tempfile;
extern crate which;

mod error;
mod mountstack;
mod tool;

use error::*;
use failure::{Fail, ResultExt};
use mountstack::{Filesystem, MountStack};
use simplelog::*;
use std::fs;
use std::path::PathBuf;
use std::process::{exit, Command, ExitStatus};
use std::str;
use std::thread;
use std::time::Duration;
use structopt::StructOpt;
use tempfile::tempdir;
use tool::Tool;

static MKINITCPIO: &'static str = "MODULES=()
BINARIES=()
FILES=()
HOOKS=(base udev block filesystems keyboard fsck)";

#[derive(Debug, Fail)]
enum ProcessError {
    #[fail(display = "Bad exit code: {}", _0)]
    BadExitCode(ExitStatus),

    #[fail(display = "Process output isn't valid UTF-8")]
    InvalidUtf8,
}

trait CommandExt {
    fn run(&mut self, context: ErrorKind) -> Result<(), Error>;
    fn run_text_output(&mut self, context: ErrorKind) -> Result<String, Error>;
}

impl CommandExt for Command {
    fn run(&mut self, context: ErrorKind) -> Result<(), Error> {
        let exit_status = self.spawn().context(context)?.wait().context(context)?;

        if !exit_status.success() {
            return Err(ProcessError::BadExitCode(exit_status)
                .context(context)
                .into());
        }

        Ok(())
    }

    fn run_text_output(&mut self, context: ErrorKind) -> Result<String, Error> {
        let output = self.output().context(context)?;

        if !output.status.success() {
            let error = str::from_utf8(&output.stderr).unwrap_or("[INVALID UTF8]");
            error!("{}", error);
            return Err(ProcessError::BadExitCode(output.status)
                .context(context)
                .into());
        }

        Ok(String::from(
            str::from_utf8(&output.stdout)
                .map_err(|_| ProcessError::InvalidUtf8)
                .context(context)?,
        ))
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

    let mount_point = tempdir().context(ErrorKind::Creation)?;

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

    info!("Mounting filesystems to {}", mount_point.path().display());
    mount_stack
        .mount(
            &PathBuf::from(format!("{}-part3", disk.display())),
            &mount_point.path(),
            Filesystem::Btrfs,
            Some("compress=zstd"),
        ).context(ErrorKind::Creation)?;

    let boot_point = mount_point.path().join("boot");
    fs::create_dir(&boot_point).context(ErrorKind::Creation)?;

    mount_stack
        .mount(
            &PathBuf::from(format!("{}-part2", disk.display())),
            &boot_point,
            Filesystem::Vfat,
            None,
        ).context(ErrorKind::Creation)?;

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
        ]).run(ErrorKind::Creation)?;

    let fstab = genfstab
        .execute()
        .arg("-U")
        .arg(mount_point.path())
        .run_text_output(ErrorKind::Creation)?
        .replace("relatime", "noatime");
    debug!("fstab:\n{}", fstab);
    fs::write(mount_point.path().join("etc/fstab"), fstab).context(ErrorKind::Creation)?;

    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&["systemctl", "enable", "NetworkManager"])
        .run(ErrorKind::Creation)?;

    info!("Generating initramfs");
    fs::write(mount_point.path().join("etc/mkinitcpio.conf"), MKINITCPIO)
        .context(ErrorKind::Creation)?;
    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&["mkinitcpio", "-p", "linux"])
        .run(ErrorKind::Creation)?;

    info!("Installing the Bootloader");
    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&["bash", "-c"])
        .arg(format!("grub-install --target=i386-pc --boot-directory /boot {} && grub-install --target=x86_64-efi --efi-directory /boot --boot-directory /boot --removable &&  grub-mkconfig -o /boot/grub/grub.cfg", disk.display()))
        .run(ErrorKind::Creation)?;

    info!("Unmounting filesystems");
    drop(mount_stack);
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
