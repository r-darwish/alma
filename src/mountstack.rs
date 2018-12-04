use super::error::{Error, ErrorKind};
use failure::Fail;
use log::{debug, warn};
use nix;
use nix::mount::{mount, umount, MsFlags};
use std::borrow::Cow;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub enum Filesystem {
    Ext4,
    Vfat,
}

impl Filesystem {
    fn to_type(self) -> &'static str {
        match self {
            Filesystem::Ext4 => "ext4",
            Filesystem::Vfat => "vfat",
        }
    }
}

pub struct MountStack<'a> {
    targets: Vec<Cow<'a, Path>>,
}

impl<'a> MountStack<'a> {
    pub fn new() -> Self {
        MountStack {
            targets: Vec::new(),
        }
    }

    #[must_use]
    pub fn mount<T: Into<Cow<'a, Path>>>(
        &mut self,
        source: &Path,
        target: T,
        filesystem: Filesystem,
        options: Option<&str>,
    ) -> nix::Result<()> {
        let target = target.into();

        debug!("Mounting {:?} ({:?}) to {:?}", source, filesystem, target);
        mount(
            Some(source),
            target.as_ref(),
            Some(filesystem.to_type()),
            MsFlags::MS_NOATIME,
            options,
        )?;
        self.targets.push(target);
        Ok(())
    }

    fn _umount(&mut self) -> Result<(), Error> {
        let mut result = Ok(());

        while let Some(target) = self.targets.pop() {
            debug!("Unmounting {}", target.display());
            if let Err(e) = umount(target.as_ref()) {
                warn!("Unable to umount {}: {}", target.display(), e);
                result = Err(Error::from(e.context(ErrorKind::UmountFailure)));
            };
        }

        result
    }

    pub fn umount(mut self) -> Result<(), Error> {
        self._umount()
    }
}

impl<'a> Drop for MountStack<'a> {
    fn drop(&mut self) {
        self._umount().ok();
    }
}
