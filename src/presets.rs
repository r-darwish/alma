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
    shared_directories: Option<Vec<PathBuf>>,
}

fn visit_dirs(dir: &Path, filevec: &mut Vec<PathBuf>) -> Result<(), io::Error> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, filevec)?;
            } else {
                if entry.path().extension() == Some(&std::ffi::OsString::from("toml")) {
                    filevec.push(entry.path());
                }
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
        scripts: &mut Vec<Script>,
        environment_variables: &mut HashSet<String>,
        path: &PathBuf,
    ) -> Result<(), ErrorKind> {
        if let Some(preset_packages) = &self.packages {
            packages.extend(preset_packages.clone());
        }

        if let Some(preset_environment_variables) = &self.environment_variables {
            environment_variables.extend(preset_environment_variables.clone());
        }

        if let Some(script_text) = &self.script {
            scripts.push(Script {
                script_text: script_text.clone(),
                shared_dirs: self
                    .shared_directories
                    .clone()
                    .map(|x| {
                        // Convert directories to absolute paths
                        // If any shared directory is not a directory then throw an error
                        x.iter()
                            .cloned()
                            .map(|y| {
                                let full_path = path.parent().unwrap().join(&y);
                                if full_path.is_dir() {
                                    Ok(full_path)
                                } else {
                                    Err(ErrorKind::Preset(format!(
                                        "Preset: {} - shared directory: {} is not directory",
                                        path.display(),
                                        y.display()
                                    )))
                                }
                            })
                            .collect::<Result<Vec<_>, ErrorKind>>()
                    })
                    .map_or(Ok(None), |r| r.map(Some))?,
            });
        }
        Ok(())
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
            if preset.is_dir() {
                // Build vector of paths to files, then sort by path name
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
                        &path,
                    )?;
                }
            } else {
                Preset::load(&preset)?.process(
                    &mut packages,
                    &mut scripts,
                    &mut environment_variables,
                    &preset,
                )?;
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
