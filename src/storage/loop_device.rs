use crate::error::{Error, ErrorKind};
use crate::tool::Tool;
use failure::ResultExt;
use log::info;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct LoopDevice {
    path: PathBuf,
    losetup: Tool,
}

impl LoopDevice {
    pub fn create(file: &Path) -> Result<Self, Error> {
        let losetup = Tool::find("losetup")?;
        let output = losetup
            .execute()
            .args(&["--find", "-P", "--show"])
            .arg(file)
            .output()
            .context(ErrorKind::Image)?;

        if !output.status.success() {
            Err(ErrorKind::Losetup(
                String::from_utf8(output.stderr).unwrap(),
            ))?
        }

        let path = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
        info!("Mounted {} to {}", file.display(), path.display());

        Ok(LoopDevice { path, losetup })
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
            .unwrap()
            .wait()
            .ok();
    }
}
