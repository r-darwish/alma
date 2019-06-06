use crate::error::{Error, ErrorKind};
use byte_unit::Byte;
use failure::ResultExt;
use std::{fmt, fs};

#[derive(Debug)]
pub struct Device {
    model: String,
    vendor: String,
    size: Byte,
    pub name: String,
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {} ({})",
            self.vendor,
            self.model,
            self.size.get_appropriate_unit(true)
        )
    }
}

fn trimmed(source: String) -> String {
    String::from(source.trim_end())
}

pub fn get_removable_devices() -> Result<Vec<Device>, Error> {
    let mut result = Vec::new();

    for entry in fs::read_dir("/sys/block").context(ErrorKind::RemoveableDevicesQuery)? {
        let entry = entry.context(ErrorKind::RemoveableDevicesQuery)?;

        let removable = fs::read_to_string(entry.path().join("removable"))
            .context(ErrorKind::RemoveableDevicesQuery)?;

        if removable != "1\n" {
            continue;
        }

        let model = fs::read_to_string(entry.path().join("device/model"))
            .map(trimmed)
            .context(ErrorKind::RemoveableDevicesQuery)?;

        if model == "CD-ROM" {
            continue;
        }

        result.push(Device {
            name: entry
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            model,
            vendor: fs::read_to_string(entry.path().join("device/vendor"))
                .map(trimmed)
                .context(ErrorKind::RemoveableDevicesQuery)?,
            size: Byte::from_bytes(
                fs::read_to_string(entry.path().join("size"))
                    .context(ErrorKind::RemoveableDevicesQuery)?
                    .trim()
                    .parse::<u128>()
                    .unwrap()
                    * 512,
            ),
        })
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanity() {
        let devices = get_removable_devices().unwrap();
        println!("{:?}", devices);
    }
}
