use super::mount;
use super::Tool;
use crate::args;
use crate::constants::{BOOT_PARTITION_INDEX, ROOT_PARTITION_INDEX};
use crate::error::{Error, ErrorKind};
use crate::process::CommandExt;
use crate::storage;
use crate::storage::{is_encrypted_device, EncryptedDevice};
use crate::storage::{BlockDevice, Filesystem, FilesystemType, LoopDevice};
use log::info;

use failure::ResultExt;
use tempfile::tempdir;

/// Use arch-chroot to chroot to the given device
/// Also handles encrypted root partitions (detected by checking for the LUKS magic header)
pub fn chroot(command: args::ChrootCommand) -> Result<(), Error> {
    let arch_chroot = Tool::find("arch-chroot")?;
    let cryptsetup;

    let loop_device: Option<LoopDevice>;
    let storage_device =
        match storage::StorageDevice::from_path(&command.block_device, command.allow_non_removable)
        {
            Ok(b) => b,
            Err(_) => {
                loop_device = Some(LoopDevice::create(&command.block_device)?);
                storage::StorageDevice::from_path(
                    loop_device.as_ref().unwrap().path(),
                    command.allow_non_removable,
                )?
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
        e as &dyn BlockDevice
    } else {
        &root_partition_base as &dyn BlockDevice
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
