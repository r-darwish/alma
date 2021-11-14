use anyhow::Context;
use byte_unit::Byte;
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

pub fn get_storage_devices(allow_non_removable: bool) -> anyhow::Result<Vec<Device>> {
    let mut result = Vec::new();

    for entry in fs::read_dir("/sys/block").context("Error querying storage devices")? {
        let entry = entry.context("Error querying storage devices")?;

        let removable = allow_non_removable
            || fs::read_to_string(entry.path().join("removable"))
                .map(|v| v == "1\n")
                .context("Error querying storage devices")?;

        if !removable {
            continue;
        }

        let model = fs::read_to_string(entry.path().join("device/model"))
            .map(trimmed)
            .context("Error querying storage devices")?;

        if model == "CD-ROM" {
            continue;
        }

        result.push(Device {
            name: entry
                .path()
                .file_name()
                .expect("Could not get file name for dir entry /sys/block")
                .to_string_lossy()
                .into_owned(),
            model,
            vendor: fs::read_to_string(entry.path().join("device/vendor"))
                .map(trimmed)
                .context("Error querying storage devices")?,
            size: Byte::from_bytes(
                fs::read_to_string(entry.path().join("size"))
                    .context("Error querying storage devices")?
                    .trim()
                    .parse::<u128>()
                    .context("Could not parse block size to unsigned integer (u128)")?
                    * 512,
            ),
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanity() {
        let devices = get_storage_devices(false).expect("No devices");
        println!("{:?}", devices);
    }
}
