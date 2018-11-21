use log::{debug, warn};
use nix;
use nix::mount::{mount, umount, MsFlags};
use std::path::Path;

#[derive(Debug)]
pub enum Filesystem {
    Btrfs,
    Vfat,
}

impl Filesystem {
    fn to_type(&self) -> &'static str {
        match self {
            Filesystem::Btrfs => "btrfs",
            Filesystem::Vfat => "vfat",
        }
    }
}

pub struct MountStack<'a> {
    targets: Vec<&'a Path>,
}

impl<'a> MountStack<'a> {
    pub fn new() -> Self {
        MountStack {
            targets: Vec::new(),
        }
    }

    #[must_use]
    pub fn mount(
        &mut self,
        source: &Path,
        target: &'a Path,
        filesystem: Filesystem,
        options: Option<&str>,
    ) -> nix::Result<()> {
        debug!("Mounting {:?} ({:?}) to {:?}", source, filesystem, target);
        mount(
            Some(source),
            target,
            Some(filesystem.to_type()),
            MsFlags::empty(),
            options,
        )?;
        self.targets.push(target);
        Ok(())
    }
}

impl<'a> Drop for MountStack<'a> {
    fn drop(&mut self) {
        while let Some(target) = self.targets.pop() {
            debug!("Unmounting {}", target.display());
            if !umount(target).is_ok() {
                warn!("Unable to mount {}", target.display());
            };
        }
    }
}
