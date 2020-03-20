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
    shared_directories: Option<Vec<PathBuf>>,
}

impl Preset {
    fn load(path: &Path) -> Result<Self, Error> {
        let data = fs::read_to_string(path)
            .with_context(|_| ErrorKind::Preset(format!("{}", path.display())))?;
        Ok(toml::from_str(&data)
            .with_context(|_| ErrorKind::Preset(format!("{}", path.display())))?)
    }
}

pub struct Script {
    pub script_text: String,
    pub shared_dirs: Option<Vec<PathBuf>>,
}

pub struct PresetsCollection {
    pub packages: HashSet<String>,
    pub scripts: Vec<Script>,
}

impl PresetsCollection {
    pub fn load(list: &[PathBuf]) -> Result<Self, Error> {
        let mut packages = HashSet::new();
        let mut scripts: Vec<Script> = Vec::new();
        let mut environment_variables = HashSet::new();

        for preset in list {
            let Preset {
                script,
                packages: preset_packages,
                environment_variables: preset_environment_variables,
                shared_directories: preset_shared_dirs,
            } = Preset::load(&preset)?;

            if let Some(preset_packages) = preset_packages {
                packages.extend(preset_packages);
            }

            if let Some(preset_environment_variables) = preset_environment_variables {
                environment_variables.extend(preset_environment_variables);
            }

            if let Some(script) = script {
                scripts.push(Script {
                    script_text: script,

                    shared_dirs: preset_shared_dirs
                        .map(|x| {
                            // Convert directories to absolute paths
                            // If any shared directory is not a directory then throw an error
                            x.iter()
                                .cloned()
                                .map(|y| {
                                    let full_path = preset.parent().unwrap().join(&y);
                                    if full_path.is_dir() {
                                        Ok(full_path)
                                    } else {
                                        Err(ErrorKind::Preset(format!(
                                            "Preset: {} - shared directory: {} is not directory",
                                            preset.to_string_lossy(),
                                            y.to_string_lossy()
                                        )))
                                    }
                                })
                                .collect::<Result<Vec<_>, ErrorKind>>()
                        })
                        .map_or(Ok(None), |r| r.map(Some))?,
                });
            }
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
