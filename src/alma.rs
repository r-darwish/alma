use super::block::BlockDevice;
use super::error::{Error, ErrorKind};
use super::mountstack::{Filesystem, MountStack};
use failure::ResultExt;
use log::info;
use std::fs;
use std::path::{Path, PathBuf};

pub struct ALMA {
    block: BlockDevice,
}

impl ALMA {
    pub fn new(block: BlockDevice) -> Self {
        Self { block }
    }

    pub fn mount<'a>(&self, path: &'a Path) -> Result<MountStack<'a>, Error> {
        let mut mount_stack = MountStack::new();

        info!("Mounting filesystems to {}", path.display());
        mount_stack
            .mount(
                &PathBuf::from(&self.block.partition_device_path(3)?),
                path,
                Filesystem::Btrfs,
                None,
            ).context(ErrorKind::Mounting)?;

        let boot_point = path.join("boot");
        if !boot_point.exists() {
            fs::create_dir(&boot_point).context(ErrorKind::CreateBoot)?;
        }

        mount_stack
            .mount(
                &self.block.partition_device_path(2)?,
                boot_point,
                Filesystem::Vfat,
                None,
            ).context(ErrorKind::Mounting)?;

        Ok(mount_stack)
    }
}
