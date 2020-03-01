use super::Filesystem;
use anyhow::Result;
use log::{debug, warn};
use nix;
use nix::mount::{mount, umount, MsFlags};
use std::marker::PhantomData;
use std::path::PathBuf;

pub struct MountStack<'a> {
    targets: Vec<PathBuf>,
    filesystems: PhantomData<Filesystem<'a>>,
}

impl<'a> MountStack<'a> {
    pub fn new() -> Self {
        MountStack {
            targets: Vec::new(),
            filesystems: PhantomData,
        }
    }

    #[must_use]
    pub fn mount(
        &mut self,
        filesystem: &'a Filesystem,
        target: PathBuf,
        options: Option<&str>,
    ) -> nix::Result<()> {
        let source = filesystem.block().path();
        debug!("Mounting {:?} to {:?}", filesystem, target);
        mount(
            Some(source),
            &target,
            Some(filesystem.fs_type().to_mount_type()),
            MsFlags::MS_NOATIME,
            options,
        )?;
        self.targets.push(target);
        Ok(())
    }

    fn _umount(&mut self) -> Result<()> {
        let mut result = Ok(());

        while let Some(target) = self.targets.pop() {
            debug!("Unmounting {}", target.display());
            if let Err(e) = umount(&target) {
                warn!("Unable to umount {}: {}", target.display(), e);
                result = Err(e.into());
            };
        }

        result
    }

    pub fn umount(mut self) -> Result<()> {
        self._umount()
    }
}

impl<'a> Drop for MountStack<'a> {
    fn drop(&mut self) {
        self._umount().ok();
    }
}
