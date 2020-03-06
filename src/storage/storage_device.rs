use super::markers::{BlockDevice, Origin};
use super::partition::Partition;
use crate::error::{Error, ErrorKind};
use failure::ResultExt;
use log::debug;
use std::fs::read_to_string;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct StorageDevice<'a> {
    name: String,
    path: PathBuf,
    origin: PhantomData<&'a dyn Origin>,
}

impl<'a> StorageDevice<'a> {
    pub fn from_path(path: &'a Path, allow_non_removable: bool) -> Result<Self, Error> {
        debug!("path: {:?}", path);
        let path = path.canonicalize().context(ErrorKind::DeviceQuery)?;
        let device_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .map(String::from)
            .ok_or_else(|| Error::from(ErrorKind::InvalidDeviceName))?;

        debug!("real path: {:?}, device name: {:?}", path, device_name);

        let _self = Self {
            name: device_name,
            path,
            origin: PhantomData,
        };

        // If we only allow removable/loop devices, and the device is neither removable or a loop
        // device then throw a DangerousDevice error
        if !(allow_non_removable || _self.is_removable_device()? || _self.is_loop_device()) {
            return Err(ErrorKind::DangerousDevice.into());
        }

        Ok(_self)
    }

    fn sys_path(&self) -> PathBuf {
        let mut path = PathBuf::from("/sys/block");
        path.push(self.name.clone());
        path
    }

    fn is_removable_device(&self) -> Result<bool, Error> {
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

    pub fn get_partition(&self, index: u8) -> Result<Partition, Error> {
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
        Ok(Partition::new::<Self>(path))
    }
}

impl<'a> BlockDevice for StorageDevice<'a> {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl<'a> Origin for StorageDevice<'a> {}
