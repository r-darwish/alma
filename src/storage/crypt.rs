use super::markers::BlockDevice;
use crate::error::{Error, ErrorKind};
use crate::process::CommandExt;
use crate::tool::Tool;
use log::{debug, warn};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct EncryptedDevice<'t, 'o> {
    cryptsetup: &'t Tool,
    name: String,
    path: PathBuf,
    origin: PhantomData<&'o BlockDevice>,
}

impl<'t, 'o> EncryptedDevice<'t, 'o> {
    pub fn prepare(cryptsetup: &Tool, device: &BlockDevice) -> Result<(), Error> {
        debug!("Preparing encrypted device in {}", device.path().display());
        cryptsetup
            .execute()
            .arg("luksFormat")
            .arg("-q")
            .arg(device.path())
            .run(ErrorKind::LuksSetup)?;

        Ok(())
    }

    pub fn open(
        cryptsetup: &'t Tool,
        device: &'o BlockDevice,
        name: String,
    ) -> Result<EncryptedDevice<'t, 'o>, Error> {
        debug!(
            "Opening encrypted device {} as {}",
            device.path().display(),
            name
        );
        cryptsetup
            .execute()
            .arg("open")
            .arg(device.path())
            .arg(&name)
            .run(ErrorKind::LuksOpen)?;

        let path = PathBuf::from("/dev/mapper").join(&name);
        Ok(Self {
            cryptsetup,
            name,
            path,
            origin: PhantomData,
        })
    }

    fn _close(&mut self) -> Result<(), Error> {
        debug!("Closing encrypted device {}", self.name);
        self.cryptsetup
            .execute()
            .arg("close")
            .arg(&self.name)
            .run(ErrorKind::LuksClose)?;

        Ok(())
    }
}

impl<'t, 'o> Drop for EncryptedDevice<'t, 'o> {
    fn drop(&mut self) {
        if self._close().is_err() {
            warn!("Error closing {}", self.name);
        }
    }
}

impl<'t, 'o> BlockDevice for EncryptedDevice<'t, 'o> {
    fn path(&self) -> &Path {
        &self.path
    }
}
