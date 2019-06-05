use crate::error::{Error, ErrorKind};
use failure::ResultExt;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use toml;

#[derive(Deserialize)]

struct Preset {
    packages: Option<Vec<String>>,
    script: Option<String>,
}

impl Preset {
    fn load(path: &Path) -> Result<Self, Error> {
        let data = fs::read_to_string(path)
            .with_context(|_| ErrorKind::Preset(format!("{}", path.display())))?;
        Ok(toml::from_str(&data)
            .with_context(|_| ErrorKind::Preset(format!("{}", path.display())))?)
    }
}

pub struct Presets {
    pub packages: HashSet<String>,
    pub scripts: Vec<String>,
}

impl Presets {
    pub fn load(list: &[PathBuf]) -> Result<Self, Error> {
        let mut packages = HashSet::new();
        let mut scripts = Vec::new();

        for preset in list {
            let Preset {
                script,
                packages: preset_packages,
            } = Preset::load(&preset)?;

            if let Some(preset_packages) = preset_packages {
                packages.extend(preset_packages);
            }

            scripts.extend(script);
        }

        Ok(Self { packages, scripts })
    }
}
