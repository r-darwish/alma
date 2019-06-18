mod args;
mod error;
mod presets;
mod process;
mod storage;
mod tool;

use crate::args::*;
use crate::error::*;
use crate::process::CommandExt;
use crate::storage::*;
use crate::tool::Tool;
use byte_unit::Byte;
use failure::{Fail, ResultExt};
use log::{debug, error, info, warn};
use nix::sys::signal;
use simplelog::*;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::os::unix::{fs::PermissionsExt, process::CommandExt as UnixCommandExt};
use std::path::Path;
use std::process::exit;
use std::thread;
use std::time::Duration;
use structopt::StructOpt;
use tempfile::tempdir;

const BOOT_PARTITION_INDEX: u8 = 1;
const ROOT_PARTITION_INDEX: u8 = 3;

static MKINITCPIO: &'static str = "MODULES=()
BINARIES=()
FILES=()
HOOKS=(base udev keyboard consolefont block encrypt filesystems keyboard fsck)";

static JOURNALD_CONF: &'static str = "
[Journal]
Storage=volatile
SystemMaxUse=16M
";

fn mount<'a>(
    mount_path: &Path,
    boot_filesystem: &'a Filesystem,
    root_filesystem: &'a Filesystem,
) -> Result<MountStack<'a>, Error> {
    let mut mount_stack = MountStack::new();
    debug!(
        "Root partition: {}",
        root_filesystem.block().path().display()
    );

    info!("Mounting filesystems to {}", mount_path.display());
    mount_stack
        .mount(&root_filesystem, mount_path.into(), None)
        .context(ErrorKind::Mounting)?;

    let boot_point = mount_path.join("boot");
    if !boot_point.exists() {
        fs::create_dir(&boot_point).context(ErrorKind::CreateBoot)?;
    }

    mount_stack
        .mount(&boot_filesystem, boot_point, None)
        .context(ErrorKind::Mounting)?;

    Ok(mount_stack)
}

fn fix_fstab(fstab: &str) -> String {
    fstab
        .lines()
        .filter(|line| !line.contains("swap") && !line.starts_with('#'))
        .collect::<Vec<&str>>()
        .join("\n")
}

fn create_image(path: &Path, size: Byte) -> Result<LoopDevice, Error> {
    {
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .context(ErrorKind::Image)?;

        file.set_len(size.get_bytes() as u64)
            .context(ErrorKind::Image)?;
    }

    LoopDevice::create(path)
}

fn create(command: CreateCommand) -> Result<(), Error> {
    let presets = presets::Presets::load(&command.presets)?;

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

    let image_loop = if let Some(size) = command.image {
        Some(create_image(&command.path, size)?)
    } else {
        None
    };

    let storage_device = storage::StorageDevice::from_path(
        image_loop
            .as_ref()
            .map(|loop_dev| {
                info!("Using loop device at {}", loop_dev.path().display());
                loop_dev.path()
            })
            .unwrap_or(&command.path),
    )?;
    let mount_point = tempdir().context(ErrorKind::TmpDirError)?;
    let disk_path = storage_device.path();

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
    let boot_partition = storage_device.get_partition(BOOT_PARTITION_INDEX)?;
    let boot_filesystem = Filesystem::format(&boot_partition, FilesystemType::Vfat, &mkfat)?;

    let root_partition_base = storage_device.get_partition(ROOT_PARTITION_INDEX)?;
    let encrypted_root = if let Some(cryptsetup) = &cryptsetup {
        info!("Encrypting the root filesystem");
        EncryptedDevice::prepare(&cryptsetup, &root_partition_base)?;
        Some(EncryptedDevice::open(
            cryptsetup,
            &root_partition_base,
            "alma_root".into(),
        )?)
    } else {
        None
    };

    let root_partition = if let Some(e) = encrypted_root.as_ref() {
        e as &BlockDevice
    } else {
        &root_partition_base as &BlockDevice
    };

    let root_filesystem = Filesystem::format(root_partition, FilesystemType::Ext4, &mkext4)?;

    let mount_stack = mount(mount_point.path(), &boot_filesystem, &root_filesystem)?;

    let mut packages: HashSet<String> = [
        "base",
        "grub",
        "efibootmgr",
        "intel-ucode",
        "networkmanager",
        "broadcom-wl",
    ]
    .iter()
    .map(|s| String::from(*s))
    .collect();

    packages.extend(presets.packages);

    info!("Bootstrapping system");
    pacstrap
        .execute()
        .arg("-c")
        .arg(mount_point.path())
        .args(packages)
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

    if !presets.scripts.is_empty() {
        info!("Running custom scripts");
    }

    for script in presets.scripts {
        let mut script_file =
            tempfile::NamedTempFile::new_in(mount_point.path()).context(ErrorKind::PresetScript)?;
        script_file
            .write_all(script.as_bytes())
            .and_then(|_| script_file.as_file_mut().metadata())
            .and_then(|metadata| {
                let mut permissions = metadata.permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(script_file.path(), permissions)
            })
            .context(ErrorKind::PresetScript)?;

        let script_path = script_file.into_temp_path();
        arch_chroot
            .execute()
            .arg(mount_point.path())
            .arg(Path::new("/").join(script_path.file_name().unwrap()))
            .run(ErrorKind::PostInstallation)?;
    }

    info!("Performing post installation tasks");

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

    if encrypted_root.is_some() {
        debug!("Setting up GRUB for an encrypted root partition");

        let uuid = blkid
            .unwrap()
            .execute()
            .arg(root_partition_base.path())
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
    let mut cryptsetup;

    let mut loop_device: Option<LoopDevice>;
    let storage_device = match storage::StorageDevice::from_path(&command.block_device) {
        Ok(b) => b,
        Err(_) => {
            loop_device = Some(LoopDevice::create(&command.block_device)?);
            storage::StorageDevice::from_path(loop_device.as_ref().unwrap().path())?
        }
    };
    let mount_point = tempdir().context(ErrorKind::TmpDirError)?;

    let boot_partition = storage_device.get_partition(BOOT_PARTITION_INDEX)?;
    let boot_filesystem = Filesystem::from_partition(&boot_partition, FilesystemType::Vfat);

    let root_partition_base = storage_device.get_partition(ROOT_PARTITION_INDEX)?;
    let encrypted_root = if is_encrypted_device(&root_partition_base)? {
        cryptsetup = Some(Tool::find("cryptsetup")?);
        Some(EncryptedDevice::open(
            cryptsetup.as_ref().unwrap(),
            &root_partition_base,
            "alma_root".into(),
        )?)
    } else {
        None
    };

    let root_partition = if let Some(e) = encrypted_root.as_ref() {
        e as &BlockDevice
    } else {
        &root_partition_base as &BlockDevice
    };
    let root_filesystem = Filesystem::from_partition(root_partition, FilesystemType::Ext4);

    let mount_stack = mount(mount_point.path(), &boot_filesystem, &root_filesystem)?;

    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&command.command)
        .run(ErrorKind::Interactive)?;

    info!("Unmounting filesystems");
    mount_stack.umount()?;

    Ok(())
}

fn qemu(command: QemuCommand) -> Result<(), Error> {
    let qemu = Tool::find("qemu-system-x86_64")?;

    let err = qemu
        .execute()
        .args(&[
            "-enable-kvm",
            "-cpu",
            "host",
            "-m",
            "4G",
            "-netdev",
            "user,id=user.0",
            "-device",
            "virtio-net-pci,netdev=user.0",
            "-device",
            "qemu-xhci,id=xhci",
            "-device",
            "usb-tablet,bus=xhci.0",
            "-drive",
        ])
        .arg(format!(
            "file={},if=virtio,format=raw",
            command.block_device.display()
        ))
        .args(command.args)
        .exec();

    Err(err).context(ErrorKind::Qemu)?
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
        Command::Qemu(command) => qemu(command),
    };

    match result {
        Ok(()) => {
            exit(0);
        }
        Err(error) => {
            error!("{}", error);
            for cause in (&error as &Fail).iter_causes() {
                error!("Caused by: {}", cause);
            }
            exit(1);
        }
    }
}
