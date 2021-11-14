use crate::storage::{Filesystem, MountStack};
use anyhow::Context;
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
) -> anyhow::Result<MountStack<'a>> {
    let mut mount_stack = MountStack::new();
    debug!(
        "Root partition: {}",
        root_filesystem.block().path().display()
    );

    info!("Mounting filesystems to {}", mount_path.display());
    mount_stack
        .mount(root_filesystem, mount_path.into(), None)
        .with_context(|| format!("Error mounting filesystem to {}", mount_path.display()))?;

    let boot_point = mount_path.join("boot");
    if !boot_point.exists() {
        fs::create_dir(&boot_point).context("Error creating the boot directory")?;
    }

    mount_stack
        .mount(boot_filesystem, boot_point, None)
        .context("Error mounting the boot point")?;

    Ok(mount_stack)
}
