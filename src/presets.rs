use crate::error::{Error, ErrorKind};
use failure::ResultExt;
use serde::Deserialize;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct Preset {
    packages: Option<Vec<String>>,
    script: Option<String>,
    environment_variables: Option<Vec<String>>,
}

impl Preset {
    fn load(path: &Path) -> Result<Self, Error> {
        let data = fs::read_to_string(path)
            .with_context(|_| ErrorKind::Preset(format!("{}", path.display())))?;
        Ok(toml::from_str(&data)
            .with_context(|_| ErrorKind::Preset(format!("{}", path.display())))?)
    }
}

pub struct PresetsCollection {
    pub packages: HashSet<String>,
    pub scripts: Vec<String>,
}

impl PresetsCollection {
    pub fn load(list: &[PathBuf]) -> Result<Self, Error> {
        let mut packages = HashSet::new();
        let mut scripts = Vec::new();
        let mut environment_variables = HashSet::new();

        for preset in list {
            let Preset {
                script,
                packages: preset_packages,
                environment_variables: preset_environment_variables,
            } = Preset::load(&preset)?;

            if let Some(preset_packages) = preset_packages {
                packages.extend(preset_packages);
            }

            if let Some(preset_environment_variables) = preset_environment_variables {
                environment_variables.extend(preset_environment_variables);
            }

            scripts.extend(script);
        }

        let missing_envrionments: Vec<String> = environment_variables
            .into_iter()
            .filter(|var| env::var(var).is_err())
            .collect();

        if !missing_envrionments.is_empty() {
            return Err(ErrorKind::MissingEnvironmentVariables(missing_envrionments).into());
        }

        Ok(Self { packages, scripts })
    }
}
