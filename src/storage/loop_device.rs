use crate::tool::Tool;
use anyhow::{anyhow, Context};
use log::info;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct LoopDevice {
    path: PathBuf,
    losetup: Tool,
}

impl LoopDevice {
    pub fn create(file: &Path) -> anyhow::Result<Self> {
        let losetup = Tool::find("losetup")?;
        let output = losetup
            .execute()
            .args(&["--find", "-P", "--show"])
            .arg(file)
            .output()
            .context("Error creating the image")?;

        if !output.status.success() {
            return Err(anyhow!(String::from_utf8(output.stderr)?));
        }

        let path = PathBuf::from(
            String::from_utf8(output.stdout)
                .context("Output not valid UTF-8")?
                .trim(),
        );
        info!("Mounted {} to {}", file.display(), path.display());

        Ok(Self { path, losetup })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for LoopDevice {
    fn drop(&mut self) {
        info!("Detaching loop device {}", self.path.display());
        self.losetup
            .execute()
            .arg("-d")
            .arg(&self.path)
            .spawn()
            .expect("Failed to spawn command to detach loop device")
            .wait()
            .ok();
    }
}
