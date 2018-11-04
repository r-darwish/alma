use nix;
use nix::mount::{mount, umount, MsFlags};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum Filesystem {
    Btrfs,
    Vfat,
}

impl Filesystem {
    fn to_type(self) -> &'static str {
        match self {
            Filesystem::Btrfs => "btrfs",
            Filesystem::Vfat => "vfat",
        }
    }
}

pub struct MountStack {
    targets: Vec<PathBuf>,
}

impl MountStack {
    pub fn new() -> Self {
        MountStack {
            targets: Vec::new(),
        }
    }

    #[must_use]
    pub fn mount(
        &mut self,
        source: &Path,
        target: &Path,
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
        self.targets.push(target.to_owned());
        Ok(())
    }
}

impl Drop for MountStack {
    fn drop(&mut self) {
        while let Some(target) = self.targets.pop() {
            if !umount(&target).is_ok() {
                warn!("Unable to mount {}", target.display());
            };
        }
    }
}
