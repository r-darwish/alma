use super::markers::BlockDevice;
use crate::{
    error::{Error, ErrorKind},
    process::CommandExt,
    tool::Tool,
};

#[derive(Debug, Clone, Copy)]
pub enum FilesystemType {
    Ext4,
    Vfat,
}

impl FilesystemType {
    pub fn to_mount_type(self) -> &'static str {
        match self {
            FilesystemType::Ext4 => "ext4",
            FilesystemType::Vfat => "vfat",
        }
    }
}

#[derive(Debug)]
pub struct Filesystem<'a> {
    fs_type: FilesystemType,
    block: &'a dyn BlockDevice,
}

impl<'a> Filesystem<'a> {
    pub fn format(
        block: &'a dyn BlockDevice,
        fs_type: FilesystemType,
        mkfs: &Tool,
    ) -> Result<Self, Error> {
        let mut command = mkfs.execute();
        match fs_type {
            FilesystemType::Ext4 => command.arg("-F").arg(block.path()),
            FilesystemType::Vfat => command.arg("-F32").arg(block.path()),
        };

        command.run(ErrorKind::Formatting)?;

        Ok(Self { fs_type, block })
    }

    pub fn from_partition(block: &'a dyn BlockDevice, fs_type: FilesystemType) -> Self {
        Self { fs_type, block }
    }

    pub fn block(&self) -> &dyn BlockDevice {
        self.block
    }

    pub fn fs_type(&self) -> FilesystemType {
        self.fs_type
    }
}
