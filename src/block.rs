use super::error::{Error, ErrorKind};
use failure::ResultExt;
use log::debug;
use std::fs::read_to_string;
use std::path::PathBuf;

#[derive(Debug)]
pub struct BlockDevice {
    name: String,
}

impl BlockDevice {
    pub fn from_path(path: PathBuf) -> Result<Self, Error> {
        let real_path = path.canonicalize().context(ErrorKind::DeviceQuery)?;
        let device_name = real_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(String::from)
            .ok_or_else(|| Error::from(ErrorKind::InvalidDeviceName))?;

        debug!(
            "path: {:?}, real path: {:?}, device name: {:?}",
            path, real_path, device_name
        );

        drop(path);
        let _self = Self { name: device_name };
        if !(_self.is_removable()? || _self.is_loop_device()) {
            return Err(ErrorKind::DangerousDevice)?;
        }

        Ok(_self)
    }

    fn sys_path(&self) -> PathBuf {
        let mut path = PathBuf::from("/sys/block");
        path.push(self.name.clone());
        path
    }

    fn is_removable(&self) -> Result<bool, Error> {
        let mut path = self.sys_path();
        path.push("removable");

        debug!("Reading: {:?}", path);
        let result = read_to_string(&path).context(ErrorKind::DeviceQuery)?;
        debug!("{:?} -> {}", path, result);

        Ok(result == "1\n")
    }

    fn is_loop_device(&self) -> bool {
        let mut path = self.sys_path();
        path.push("loop");
        path.exists()
    }

    pub fn device_path(&self) -> PathBuf {
        let mut path = PathBuf::from("/dev");
        path.push(self.name.clone());
        path
    }

    pub fn partition_device_path(&self, index: u8) -> Result<PathBuf, Error> {
        let name = if self.name.chars().rev().next().unwrap().is_digit(10) {
            format!("{}p{}", self.name, index)
        } else {
            format!("{}{}", self.name, index)
        };
        let mut path = PathBuf::from("/dev");
        path.push(name);

        debug!("Partition {} for {} is in {:?}", index, self.name, path);
        if !path.exists() {
            return Err(ErrorKind::NoSuchPartition(index).into());
        }
        Ok(path)
    }
}
