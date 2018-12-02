use super::error::{Error, ErrorKind};
use super::process::CommandExt;
use super::tool::Tool;
use log::{debug, warn};
use std::path::{Path, PathBuf};

pub struct EncryptedDevice<'a> {
    cryptsetup: &'a Tool,
    name: &'a str,
    path: PathBuf,
}

impl<'a> EncryptedDevice<'a> {
    pub fn prepare(cryptsetup: &Tool, device: &Path) -> Result<(), Error> {
        debug!("Preparing encrypted device in {}", device.display());
        cryptsetup
            .execute()
            .arg("luksFormat")
            .arg("-q")
            .arg(device)
            .run(ErrorKind::LuksSetup)?;

        Ok(())
    }

    pub fn open(
        cryptsetup: &'a Tool,
        device: &Path,
        name: &'a str,
    ) -> Result<EncryptedDevice<'a>, Error> {
        debug!("Opening encrypted device {} as {}", device.display(), name);
        cryptsetup
            .execute()
            .arg("open")
            .arg(device)
            .arg(name)
            .run(ErrorKind::LuksOpen)?;

        Ok(Self {
            cryptsetup,
            name,
            path: PathBuf::from("/dev/mapper").join(name),
        })
    }

    fn _close(&mut self) -> Result<(), Error> {
        debug!("Closing encrypted device {}", self.name);
        self.cryptsetup
            .execute()
            .arg("close")
            .arg(self.name)
            .run(ErrorKind::LuksClose)?;

        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl<'a> Drop for EncryptedDevice<'a> {
    fn drop(&mut self) {
        if self._close().is_err() {
            warn!("Error closing {}", self.name);
        }
    }
}
