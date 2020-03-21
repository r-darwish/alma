mod args;
mod constants;
mod error;
mod initcpio;
mod presets;
mod process;
mod storage;
mod tool;

use args::Command;
use byte_unit::Byte;
use console::style;
use dialoguer::{theme::ColorfulTheme, Select};
use error::Error;
use error::ErrorKind;
use failure::{Fail, ResultExt};
use log::{debug, error, info, log_enabled, Level, LevelFilter};
use process::CommandExt;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{exit, Command as ProcessCommand};
use std::thread;
use std::time::Duration;
use storage::EncryptedDevice;
use storage::{BlockDevice, Filesystem, FilesystemType, LoopDevice, MountStack};
use structopt::StructOpt;
use tempfile::tempdir;
use tool::Tool;

fn main() {
    // Get struct of args using structopt
    let app = args::App::from_args();

    // Set up logging
    let mut builder = pretty_env_logger::formatted_timed_builder();
    let log_level = if app.verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    builder.filter_level(log_level);
    builder.init();

    // Match command from arguments and run relevant code
    let result = match app.cmd {
        Command::Create(command) => create(command),
        Command::Chroot(command) => tool::chroot(command),
        Command::Qemu(command) => tool::qemu(command),
    };

    // Check if command return an Error
    // Print all causes to stderr if so
    match result {
        Ok(()) => {
            exit(0);
        }
        Err(error) => {
            error!("{}", error);
            for cause in (&error as &dyn Fail).iter_causes() {
                error!("Caused by: {}", cause);
            }
            exit(1);
        }
    }
}

/// Remove swap entry from fstab and any commented lines
/// Returns an owned String
///
/// # Arguments
/// * `fstab` - A string slice holding the contents of the fstab file
fn fix_fstab(fstab: &str) -> String {
    fstab
        .lines()
        .filter(|line| !line.contains("swap") && !line.starts_with('#'))
        .collect::<Vec<&str>>()
        .join("\n")
}

/// Creates a file at the path provided, and mounts it to a loop device
fn create_image(path: &Path, size: Byte, overwrite: bool) -> Result<LoopDevice, Error> {
    {
        let mut options = fs::OpenOptions::new();

        options.write(true);
        if overwrite {
            options.create(true);
        } else {
            options.create_new(true);
        }
        let file = options.open(path).context(ErrorKind::Image)?;

        file.set_len(size.get_bytes() as u64)
            .context(ErrorKind::Image)?;
    }

    LoopDevice::create(path)
}

/// Requests selection of block device (no device was given in the arguments)
fn select_block_device(allow_non_removable: bool) -> Result<PathBuf, Error> {
    let devices = storage::get_storage_devices(allow_non_removable)?;

    if devices.is_empty() {
        return Err(ErrorKind::NoRemovableDevices.into());
    }

    if allow_non_removable {
        println!(
            "{}\n",
            style("Showing non-removable devices. Make sure you select the correct device.")
                .red()
                .bold()
        );
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a removable device")
        .default(0)
        .items(&devices)
        .interact()
        .unwrap();

    Ok(PathBuf::from("/dev").join(&devices[selection].name))
}

/// Creates the installation
#[allow(clippy::cognitive_complexity)] // TODO: Split steps into functions and remove this
fn create(command: args::CreateCommand) -> Result<(), Error> {
    let presets = presets::PresetsCollection::load(&command.presets)?;

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

    let storage_device_path = if let Some(path) = command.path {
        path
    } else {
        select_block_device(command.allow_non_removable)?
    };

    let image_loop = if let Some(size) = command.image {
        Some(create_image(&storage_device_path, size, command.overwrite)?)
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
            .unwrap_or(&storage_device_path),
        command.allow_non_removable,
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
    let boot_partition = storage_device.get_partition(constants::BOOT_PARTITION_INDEX)?;
    let boot_filesystem = Filesystem::format(&boot_partition, FilesystemType::Vfat, &mkfat)?;

    let root_partition_base = storage_device.get_partition(constants::ROOT_PARTITION_INDEX)?;
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
        e as &dyn BlockDevice
    } else {
        &root_partition_base as &dyn BlockDevice
    };

    let root_filesystem = Filesystem::format(root_partition, FilesystemType::Ext4, &mkext4)?;

    let mount_stack = tool::mount(mount_point.path(), &boot_filesystem, &root_filesystem)?;

    if log_enabled!(Level::Debug) {
        debug!("lsblk:");
        ProcessCommand::new("lsblk")
            .arg("--fs")
            .spawn()
            .and_then(|mut p| p.wait())
            .map_err(|e| {
                error!("Error running lsblk: {}", e);
            })
            .ok();
    }

    let mut packages: HashSet<String> = constants::BASE_PACKAGES
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
        let mut bind_mount_stack = MountStack::new();
        if let Some(shared_dirs) = &script.shared_dirs {
            for dir in shared_dirs {
                // Create shared directories mount points inside chroot
                std::fs::create_dir_all(
                    mount_point
                        .path()
                        .join(PathBuf::from("shared_dirs/"))
                        .join(dir.file_name().unwrap()),
                )
                .context(ErrorKind::PresetScript)?;

                // Bind mount shared directories
                let target = mount_point
                    .path()
                    .join(PathBuf::from("shared_dirs/"))
                    .join(dir.file_name().unwrap());
                bind_mount_stack
                    .bind_mount(dir.clone(), target, None)
                    .context(ErrorKind::Mounting)?;
            }
        }

        let mut script_file =
            tempfile::NamedTempFile::new_in(mount_point.path()).context(ErrorKind::PresetScript)?;
        script_file
            .write_all(script.script_text.as_bytes())
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
        constants::JOURNALD_CONF,
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
    fs::write(
        mount_point.path().join("etc/mkinitcpio.conf"),
        initcpio::Initcpio::new(encrypted_root.is_some()).to_config(),
    )
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

    debug!(
        "GRUB configuration: {}",
        fs::read_to_string(mount_point.path().join("boot/grub/grub.cfg"))
            .unwrap_or_else(|e| e.to_string())
    );

    if command.interactive {
        info!("Dropping you to chroot. Do as you wish to customize the installation. Please exit by typing 'exit' instead of using Ctrl+D");
        arch_chroot
            .execute()
            .arg(mount_point.path())
            .run(ErrorKind::Interactive)?;
    }

    info!("Unmounting filesystems");
    mount_stack.umount()?;

    Ok(())
}
