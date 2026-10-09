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
    pub cwd: Option<PathBuf>,

    #[serde(default = "default_autostart")]
    pub autostart: bool,
}

fn default_autostart() -> bool {
    true
}

pub fn load(path: &Path) -> Result<Config, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;

    let mut config: Config = toml::from_str(&content)
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

    let config_path = path
        .canonicalize()
        .map_err(|error| format!("Failed to resolve config path {}: {error}", path.display()))?;

    let config_dir = config_path
        .parent()
        .ok_or_else(|| "Config path must have a parent directory.".to_string())?;

    for (id, project) in &mut config.projects {
        let candidate = if project.path.is_absolute() {
            project.path.clone()
        } else {
            config_dir.join(&project.path)
        };

        let resolved = candidate.canonicalize().map_err(|error| {
            format!(
                "Failed to resolve project '{id}' path '{}' from {}: {error}",
                project.path.display(),
                config_dir.display()
            )
        })?;

        if !resolved.is_dir() {
            return Err(format!(
                "Project '{id}' path is not a directory: {}",
                resolved.display()
            ));
        }

        project.path = resolved;
    }

    for (id, service) in &mut config.services {
        let project = config.projects.get(&service.project).ok_or_else(|| {
            format!(
                "Service '{id}' references unknown project '{}'.",
                service.project
            )
        })?;

        let candidate = match &service.cwd {
            None => project.path.clone(),
            Some(path) => {
                if path.as_os_str().is_empty() {
                    return Err(format!(
                        "Service '{id}' working directory must not be empty."
                    ));
                }
                if path.is_absolute() {
                    path.clone()
                } else {
                    project.path.join(path)
                }
            }
        };

        let resolved = candidate.canonicalize().map_err(|error| {
            format!(
                "Failed to resolve service '{id}' working directory {}: {error}",
                candidate.display()
            )
        })?;

        if !resolved.is_dir() {
            return Err(format!(
                "Service '{id}' working directory is not a directory: {}",
                resolved.display()
            ));
        }

        service.cwd = Some(resolved);
    }

    Ok(config)
}
