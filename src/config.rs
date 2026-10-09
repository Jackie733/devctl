use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub workspace: Workspace,

    #[serde(default)]
    pub projects: BTreeMap<String, Project>,

    #[serde(default)]
    pub services: BTreeMap<String, Service>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    pub project: String,
    pub cmd: Vec<String>,

    #[serde(default = "default_autostart")]
    pub autostart: bool,
}

fn default_autostart() -> bool {
    true
}

pub fn load(path: &Path) -> Result<Config, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;

    let config: Config = toml::from_str(&content)
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?;

    if config.version != 1 {
        return Err(format!(
            "Unsupported config version: {}. Expected 1",
            config.version
        ));
    }

    if config.workspace.name.trim().is_empty() {
        return Err("Workspace name must not be empty.".to_string());
    }

    for (id, project) in &config.projects {
        if id.trim().is_empty() {
            return Err("Project ID must not be empty.".to_string());
        }

        if project.path.as_os_str().is_empty() {
            return Err(format!("Project '{id}' must have a non-empty path."));
        }
    }

    for (id, service) in &config.services {
        if id.trim().is_empty() {
            return Err("Service ID must not be empty.".to_string());
        }

        if !config.projects.contains_key(&service.project) {
            return Err(format!(
                "Service '{id}' references unknown project '{}'.",
                service.project
            ));
        }

        if service.cmd.is_empty() || service.cmd[0].trim().is_empty() {
            return Err(format!(
                "Service '{id}' must have a non-empty command executable."
            ));
        }
    }

    Ok(config)
}
