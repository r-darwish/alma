mod args;
mod aur;
mod constants;
mod initcpio;
mod presets;
mod process;
mod storage;
mod tool;

use anyhow::{anyhow, Context};
use args::Command;
use byte_unit::Byte;
use console::style;
use dialoguer::{theme::ColorfulTheme, Select};
use log::{debug, error, info, log_enabled, Level, LevelFilter};
use process::CommandExt;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::thread;
use std::time::Duration;
use storage::EncryptedDevice;
use storage::{BlockDevice, Filesystem, FilesystemType, LoopDevice, MountStack};
use structopt::StructOpt;
use tempfile::tempdir;
use tool::Tool;

fn main() -> anyhow::Result<()> {
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
    match app.cmd {
        Command::Create(command) => create(command),
        Command::Chroot(command) => tool::chroot(command),
        Command::Qemu(command) => tool::qemu(command),
    }?;

    Ok(())
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
fn create_image(path: &Path, size: Byte, overwrite: bool) -> anyhow::Result<LoopDevice> {
    {
        let mut options = fs::OpenOptions::new();

        options.write(true);
        if overwrite {
            options.create(true);
        } else {
            options.create_new(true);
        }
        let file = options.open(path).context("Error creating the image")?;

        file.set_len(size.get_bytes() as u64)
            .context("Error creating the image")?;
    }

    LoopDevice::create(path)
}

/// Requests selection of block device (no device was given in the arguments)
fn select_block_device(allow_non_removable: bool) -> anyhow::Result<PathBuf> {
    let devices = storage::get_storage_devices(allow_non_removable)?;

    if devices.is_empty() {
        return Err(anyhow!("There are no removable devices"));
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
        .interact()?;

    Ok(PathBuf::from("/dev").join(&devices[selection].name))
}

/// Creates the installation
#[allow(clippy::cognitive_complexity)] // TODO: Split steps into functions and remove this
fn create(command: args::CreateCommand) -> anyhow::Result<()> {
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

    let mount_point = tempdir().context("Error creating a temporary directory")?;
    let disk_path = storage_device.path();

    info!("Partitioning the block device");
    debug!("{:?}", disk_path);

    sgdisk
        .execute()
        .args(&[
            "-Z",
            "-o",
            "--new=1::+250M",
            "--new=2::+1M",
            "--largest-new=3",
            "--typecode=1:EF00",
            "--typecode=2:EF02",
        ])
        .arg(&disk_path)
        .run()
        .context("Partitioning error")?;

    thread::sleep(Duration::from_millis(1000));

    info!("Formatting filesystems");
    let boot_partition = storage_device.get_partition(constants::BOOT_PARTITION_INDEX)?;
    let boot_filesystem = Filesystem::format(&boot_partition, FilesystemType::Vfat, &mkfat)?;

    let root_partition_base = storage_device.get_partition(constants::ROOT_PARTITION_INDEX)?;
    let encrypted_root = if let Some(cryptsetup) = &cryptsetup {
        info!("Encrypting the root filesystem");
        EncryptedDevice::prepare(cryptsetup, &root_partition_base)?;
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

    let aur_pacakges = {
        let mut p = vec![String::from("shim-signed")];
        p.extend(presets.aur_packages);
        p.extend(command.aur_packages);
        p
    };

    packages.extend(constants::AUR_DEPENDENCIES.iter().map(|s| String::from(*s)));

    let pacman_conf_path = command
        .pacman_conf
        .unwrap_or_else(|| "/etc/pacman.conf".into());

    info!("Bootstrapping system");
    pacstrap
        .execute()
        .arg("-C")
        .arg(&pacman_conf_path)
        .arg("-c")
        .arg(mount_point.path())
        .args(packages)
        .args(&command.extra_packages)
        .run()
        .context("Pacstrap error")?;

    // Copy pacman.conf to the image.
    fs::copy(pacman_conf_path, mount_point.path().join("etc/pacman.conf"))
        .context("Failed copying pacman.conf")?;

    let fstab = fix_fstab(
        &genfstab
            .execute()
            .arg("-U")
            .arg(mount_point.path())
            .run_text_output()
            .context("fstab error")?,
    );
    debug!("fstab:\n{}", fstab);
    fs::write(mount_point.path().join("etc/fstab"), fstab).context("fstab error")?;

    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&["passwd", "-d", "root"])
        .run()
        .context("Failed to delete the root password")?;

    info!("Setting locale");
    fs::OpenOptions::new()
        .append(true)
        .write(true)
        .open(mount_point.path().join("etc/locale.gen"))
        .and_then(|mut locale_gen| locale_gen.write_all(b"en_US.UTF-8 UTF-8\n"))
        .context("Failed to create locale.gen")?;
    fs::write(
        mount_point.path().join("etc/locale.conf"),
        "LANG=en_US.UTF-8",
    )
    .context("Failed to write to locale.conf")?;
    arch_chroot
        .execute()
        .arg(mount_point.path())
        .arg("locale-gen")
        .run()
        .context("locale-gen failed")?;

    info!("Installing AUR packages");

    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&["useradd", "-m", "aur"])
        .run()
        .context("Failed to create temporary user to install AUR packages")?;

    let aur_sudoers = mount_point.path().join("etc/sudoers.d/aur");
    fs::write(&aur_sudoers, "aur ALL=(ALL) NOPASSWD: ALL")
        .context("Failed to modify sudoers file for AUR packages")?;

    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&["sudo", "-u", "aur"])
        .arg("git")
        .arg("clone")
        .arg(format!(
            "https://aur.archlinux.org/{}.git",
            &command.aur_helper.package_name
        ))
        .arg(format!("/home/aur/{}", &command.aur_helper.name))
        .run()
        .context("Failed to clone AUR helper package")?;

    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&[
            "bash",
            "-c",
            &format!(
                "cd /home/aur/{} && sudo -u aur makepkg -s -i --noconfirm",
                &command.aur_helper.name
            ),
        ])
        .run()
        .context("Failed to build AUR helper")?;

    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&["sudo", "-u", "aur"])
        .args(&command.aur_helper.install_command)
        .args(aur_pacakges)
        .run()
        .context("Failed to install AUR packages")?;

    // Clean up aur user:
    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&["userdel", "-r", "aur"])
        .run()
        .context("Failed to delete temporary aur user")?;

    fs::remove_file(&aur_sudoers).context("Cannot delete the AUR sudoers temporary file")?;

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
                        .join(dir.file_name().expect("Dir had no filename")),
                )
                .context("Failed mounting shared directories in preset")?;

                // Bind mount shared directories
                let target = mount_point
                    .path()
                    .join(PathBuf::from("shared_dirs/"))
                    .join(dir.file_name().expect("Dir had no filename"));
                bind_mount_stack
                    .bind_mount(dir.clone(), target, None)
                    .context("Failed mounting shared directories in preset")?;
            }
        }

        let mut script_file = tempfile::NamedTempFile::new_in(mount_point.path())
            .context("Failed creating temporary preset script")?;
        script_file
            .write_all(script.script_text.as_bytes())
            .and_then(|_| script_file.as_file_mut().metadata())
            .and_then(|metadata| {
                let mut permissions = metadata.permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(script_file.path(), permissions)
            })
            .context("Failed creating temporary preset script")?;

        let script_path = script_file.into_temp_path();
        arch_chroot
            .execute()
            .arg(mount_point.path())
            .arg(
                Path::new("/").join(
                    script_path
                        .file_name()
                        .expect("Script path had no file name"),
                ),
            )
            .run()
            .with_context(|| format!("Failed running preset script:\n{}", script.script_text))?;
    }

    info!("Performing post installation tasks");

    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&["systemctl", "enable", "NetworkManager"])
        .run()
        .context("Failed to enable NetworkManager")?;

    info!("Configuring journald");
    fs::write(
        mount_point.path().join("etc/systemd/journald.conf"),
        constants::JOURNALD_CONF,
    )
    .context("Failed to write to journald.conf")?;

    info!("Generating initramfs");
    fs::write(
        mount_point.path().join("etc/mkinitcpio.conf"),
        initcpio::Initcpio::new(encrypted_root.is_some()).to_config()?,
    )
    .context("Failed to write to mkinitcpio.conf")?;
    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&["mkinitcpio", "-p", "linux"])
        .run()
        .context("Failed to run mkinitcpio - do you have the base and linux packages installed?")?;

    if encrypted_root.is_some() {
        debug!("Setting up GRUB for an encrypted root partition");

        let uuid = blkid
            .expect("No tool for blkid")
            .execute()
            .arg(root_partition_base.path())
            .args(&["-o", "value", "-s", "UUID"])
            .run_text_output()
            .context("Failed to run blkid")?;
        let trimmed = uuid.trim();
        debug!("Root partition UUID: {}", trimmed);

        let mut grub_file = fs::OpenOptions::new()
            .append(true)
            .open(mount_point.path().join("etc/default/grub"))
            .context("Failed to create /etc/default/grub")?;

        write!(
            &mut grub_file,
            "GRUB_CMDLINE_LINUX=\"cryptdevice=UUID={}:luks_root\"",
            trimmed
        )
        .context("Failed to write to /etc/default/grub")?;
    }

    info!("Installing the Bootloader");
    arch_chroot
        .execute()
        .arg(mount_point.path())
        .args(&["bash", "-c"])
        .arg(format!("grub-install --target=i386-pc --boot-directory /boot {} && grub-install --target=x86_64-efi --efi-directory /boot --boot-directory /boot --removable &&  grub-mkconfig -o /boot/grub/grub.cfg", disk_path.display()))
        .run().context("Failed to install grub")?;

    let bootloader = mount_point.path().join("boot/EFI/BOOT/BOOTX64.efi");
    fs::rename(
        &bootloader,
        mount_point.path().join("boot/EFI/BOOT/grubx64.efi"),
    )
    .context("Cannot move out grub")?;
    fs::copy(
        mount_point.path().join("usr/share/shim-signed/mmx64.efi"),
        mount_point.path().join("boot/EFI/BOOT/mmx64.efi"),
    )
    .context("Failed copying mmx64")?;
    fs::copy(
        mount_point.path().join("usr/share/shim-signed/shimx64.efi"),
        bootloader,
    )
    .context("Failed copying shim")?;

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
            .run()
            .context("Failed to enter interactive chroot")?;
    }

    info!("Unmounting filesystems");
    mount_stack.umount()?;

    Ok(())
}
