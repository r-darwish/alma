use crate::error::{Error, ErrorKind};
use failure::ResultExt;
use serde::Deserialize;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct Preset {
    packages: Option<Vec<String>>,
    script: Option<String>,
    environment_variables: Option<Vec<String>>,
}

// TODO Build vector of paths to files, then sort by path name
fn visit_dirs(dir: &Path, filevec: &mut Vec<PathBuf>) -> Result<(), io::Error> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, filevec)?;
            } else {
                filevec.push(entry.path());
            }
        }
    }
    Ok(())
}

impl Preset {
    fn load(path: &Path) -> Result<Self, Error> {
        let data = fs::read_to_string(path)
            .with_context(|_| ErrorKind::Preset(format!("{}", path.display())))?;
        Ok(toml::from_str(&data)
            .with_context(|_| ErrorKind::Preset(format!("{}", path.display())))?)
    }

    fn process(
        &self,
        packages: &mut HashSet<String>,
        scripts: &mut Vec<String>,
        environment_variables: &mut HashSet<String>,
    ) {
        if let Some(preset_packages) = &self.packages {
            packages.extend(preset_packages.clone());
        }

        if let Some(preset_environment_variables) = &self.environment_variables {
            environment_variables.extend(preset_environment_variables.clone());
        }

        scripts.extend(self.script.clone());
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
            if preset.is_dir() {
                // Recursively load directories of preset files
                let mut dir_paths: Vec<PathBuf> = Vec::new();
                visit_dirs(&preset, &mut dir_paths)
                    .with_context(|_| ErrorKind::Preset(format!("{}", preset.display())))?;

                // Order not guaranteed so we sort
                // In the future may want to support numerical sort i.e. 15_... < 100_...
                dir_paths.sort();

                for path in dir_paths {
                    Preset::load(&path)?.process(
                        &mut packages,
                        &mut scripts,
                        &mut environment_variables,
                    );
                }
            } else {
                Preset::load(&preset)?.process(
                    &mut packages,
                    &mut scripts,
                    &mut environment_variables,
                );
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
