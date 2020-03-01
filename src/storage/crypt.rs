use super::markers::BlockDevice;
use crate::error::{Error, ErrorKind};
use crate::process::CommandExt;
use crate::tool::Tool;
use failure::ResultExt;
use log::{debug, warn};
use std::fs;
use std::io::Read;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

static LUKS_MAGIC_1: &'static [u8] = &[0x4c, 0x55, 0x4b, 0x53, 0xba, 0xbe];
static LUKS_MAGIC_2: &'static [u8] = &[0x53, 0x4b, 0x55, 0x4c, 0xba, 0xbe];

#[derive(Debug)]
pub struct EncryptedDevice<'t, 'o> {
    cryptsetup: &'t Tool,
    name: String,
    path: PathBuf,
    origin: PhantomData<&'o dyn BlockDevice>,
}

impl<'t, 'o> EncryptedDevice<'t, 'o> {
    pub fn prepare(cryptsetup: &Tool, device: &dyn BlockDevice) -> Result<(), Error> {
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
        device: &'o dyn BlockDevice,
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

pub fn is_encrypted_device(device: &dyn BlockDevice) -> Result<bool, Error> {
    let mut f = fs::OpenOptions::new()
        .read(true)
        .write(false)
        .open(device.path())
        .context(ErrorKind::LuksDetection)?;

    let mut buffer = [0; 6];
    f.read_exact(&mut buffer)
        .context(ErrorKind::LuksDetection)?;

    Ok(buffer == LUKS_MAGIC_1 || buffer == LUKS_MAGIC_2)
}
