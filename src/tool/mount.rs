use crate::error::{Error, ErrorKind};
use crate::storage::{Filesystem, MountStack};
use failure::ResultExt;
use log::{debug, info};
use std::fs;
use std::path::Path;

/// Mounts root filesystem to given mount_path
/// Mounts boot filesystem to mount_path/boot
/// Note we mount with noatime to reduce disk writes by not recording file access times
pub fn mount<'a>(
    mount_path: &Path,
    boot_filesystem: &'a Filesystem,
    root_filesystem: &'a Filesystem,
) -> Result<MountStack<'a>, Error> {
    let mut mount_stack = MountStack::new();
    debug!(
        "Root partition: {}",
        root_filesystem.block().path().display()
    );

    info!("Mounting filesystems to {}", mount_path.display());
    mount_stack
        .mount(&root_filesystem, mount_path.into(), None)
        .context(ErrorKind::Mounting)?;

    let boot_point = mount_path.join("boot");
    if !boot_point.exists() {
        fs::create_dir(&boot_point).context(ErrorKind::CreateBoot)?;
    }

    mount_stack
        .mount(&boot_filesystem, boot_point, None)
        .context(ErrorKind::Mounting)?;

    Ok(mount_stack)
}
