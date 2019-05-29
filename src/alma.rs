use super::block::BlockDevice;
use super::cryptsetup::EncryptedDevice;
use super::error::{Error, ErrorKind};
use super::mountstack::{Filesystem, MountStack};
use failure::ResultExt;
use log::{debug, info};
use std::fs;
use std::path::{Path, PathBuf};

pub struct ALMA<'a> {
    block: BlockDevice,
    encrypted_root: Option<EncryptedDevice<'a>>,
}

impl<'a> ALMA<'a> {
    pub fn new(block: BlockDevice, encrypted_root: Option<EncryptedDevice<'a>>) -> Self {
        Self {
            block,
            encrypted_root,
        }
    }

    pub fn mount<'b>(&self, path: &'b Path) -> Result<MountStack<'b>, Error> {
        let mut mount_stack = MountStack::new();

        let root_device = if let Some(encrypted_root) = &self.encrypted_root {
            PathBuf::from(encrypted_root.path())
        } else {
            self.block.partition_device_path(3)?
        };
        debug!("Root partition: {}", root_device.display());

        info!("Mounting filesystems to {}", path.display());
        mount_stack
            .mount(&root_device, path, Filesystem::Ext4, None)
            .context(ErrorKind::Mounting)?;

        let boot_point = path.join("boot");
        if !boot_point.exists() {
            fs::create_dir(&boot_point).context(ErrorKind::CreateBoot)?;
        }

        mount_stack
            .mount(
                &self.block.partition_device_path(1)?,
                boot_point,
                Filesystem::Vfat,
                None,
            )
            .context(ErrorKind::Mounting)?;

        Ok(mount_stack)
    }
}
