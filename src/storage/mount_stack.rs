use super::Filesystem;
use crate::error::{Error, ErrorKind};
use failure::Fail;
use log::{debug, warn};
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

    pub fn bind_mount(
        &mut self,
        source: PathBuf,
        target: PathBuf,
        options: Option<&str>,
    ) -> nix::Result<()> {
        debug!("Mounting {:?} to {:?}", source, target);
        mount::<_, _, str, _>(
            Some(&source),
            &target,
            None,
            MsFlags::MS_BIND | MsFlags::MS_NOATIME | MsFlags::MS_RDONLY,
            options,
        )?;
        self.targets.push(target);
        Ok(())
    }

    fn _umount(&mut self) -> Result<(), Error> {
        let mut result = Ok(());

        while let Some(target) = self.targets.pop() {
            debug!("Unmounting {}", target.display());
            if let Err(e) = umount(&target) {
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
