use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_json::Value;

use super::{Manager, files::Reader, safe};

#[derive(Debug, Clone)]
pub(crate) struct Evidence {
    pub path: PathBuf,
    pub field: String,
    pub value: String,
}

pub(super) struct Package {
    pub name: Option<String>,
    pub scripts: BTreeMap<String, String>,
}

pub(super) struct Directory {
    pub path: PathBuf,
    pub package: Option<Package>,
    pub workspace: Option<Vec<String>>,
    pub managers: Vec<Manager>,
    pub evidence: Vec<Evidence>,
    pub problems: Vec<String>,
    pub warnings: Vec<String>,
}

// Ignored YAML values are skipped by serde; do not expand an arbitrary Value tree.
#[derive(Deserialize)]
struct PnpmWorkspace {
    packages: Option<Vec<String>>,
    #[serde(rename = "useNodeVersion")]
    node_version: Option<String>,
}

pub(super) fn read(reader: &mut Reader, path: &Path, boundary: &Path) -> Result<Directory, String> {
    let mut directory = Directory {
        path: path.into(),
        package: None,
        workspace: None,
        managers: Vec::new(),
        evidence: Vec::new(),
        problems: Vec::new(),
        warnings: Vec::new(),
    };
    let manifest = path.join("package.json");
    if let Some(text) = reader.read(&manifest, boundary, false)? {
        let value: Value = serde_json::from_str(&text)
            .map_err(|error| format!("Invalid {}: {error}", safe(manifest.display())))?;
        let object = value
            .as_object()
            .ok_or_else(|| format!("Expected an object in {}", safe(manifest.display())))?;
        let name = object
            .get("name")
            .map(|name| {
                name.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("Invalid name in {}", safe(manifest.display())))
            })
            .transpose()?;
        let mut scripts = BTreeMap::new();
        if let Some(declarations) = object.get("scripts") {
            let declarations = declarations
                .as_object()
                .ok_or_else(|| format!("Invalid scripts object in {}", safe(manifest.display())))?;
            for (name, command) in declarations {
                let command = command.as_str().ok_or_else(|| {
                    format!("Invalid script {name:?} in {}", safe(manifest.display()))
                })?;
                if name.is_empty() {
                    return Err(format!("Empty script name in {}", safe(manifest.display())));
                }
                scripts.insert(name.clone(), command.into());
            }
        }
        if let Some(workspaces) = object.get("workspaces") {
            directory.workspace =
                Some(serde_json::from_value(workspaces.clone()).map_err(|_| {
                    format!(
                        "Unsupported workspaces declaration in {}; expected an array of patterns",
                        safe(manifest.display())
                    )
                })?);
            directory.evidence.push(Evidence {
                path: manifest.clone(),
                field: "workspaces".into(),
                value: workspaces.to_string(),
            });
        }
        if let Some(declaration) = object.get("packageManager") {
            let text = declaration
                .as_str()
                .ok_or_else(|| format!("Invalid packageManager in {}", safe(manifest.display())))?;
            directory.evidence.push(Evidence {
                path: manifest.clone(),
                field: "packageManager".into(),
                value: text.into(),
            });
            match parse_manager(text, &manifest, "packageManager") {
                Ok(manager) => directory.managers.push(manager),
                Err(problem) => directory.problems.push(problem),
            }
        }
        if let Some(dev_engines) = object.get("devEngines") {
            directory.evidence.push(Evidence {
                path: manifest.clone(),
                field: "devEngines".into(),
                value: dev_engines.to_string(),
            });
            if let Some(manager) = dev_engines.get("packageManager") {
                let policy = manager
                    .get("onFail")
                    .and_then(Value::as_str)
                    .unwrap_or("error");
                if matches!(policy, "warn" | "ignore") {
                    directory.warnings.push(format!(
                        "Advisory devEngines.packageManager ({policy}) in {}; evidence retained without enforcing a manager constraint.",
                        safe(manifest.display())
                    ));
                } else if policy != "error" {
                    directory.problems.push(format!(
                        "Unknown devEngines onFail policy in {}",
                        safe(manifest.display())
                    ));
                } else if let Some(name @ ("npm" | "pnpm")) =
                    manager.get("name").and_then(Value::as_str)
                {
                    match manager.get("version") {
                        None => directory.managers.push(Manager {
                            name: name.into(),
                            version: None,
                            source: manifest.clone(),
                            field: "devEngines.packageManager.name".into(),
                        }),
                        Some(Value::String(version)) if semver::Version::parse(version).is_ok() => {
                            match parse_manager(
                                &format!("{name}@{version}"),
                                &manifest,
                                "devEngines.packageManager",
                            ) {
                                Ok(manager) => directory.managers.push(manager),
                                Err(problem) => directory.problems.push(problem),
                            }
                        }
                        Some(Value::String(_)) => {
                            directory.managers.push(Manager {
                                name: name.into(),
                                version: None,
                                source: manifest.clone(),
                                field: "devEngines.packageManager.name".into(),
                            });
                            directory.warnings.push(format!("devEngines.packageManager version range in {} is recorded but not resolved by this read-only discovery step.", safe(manifest.display())));
                        }
                        Some(_) => directory.problems.push(format!(
                            "Unknown devEngines.packageManager version in {}",
                            safe(manifest.display())
                        )),
                    }
                } else {
                    directory.problems.push(format!(
                        "Unknown/unsupported devEngines.packageManager shape/name in {}",
                        safe(manifest.display())
                    ));
                }
            }
        }
        for key in ["engines", "volta", "executionEnv"] {
            if let Some(value) = object.get(key) {
                directory.evidence.push(Evidence {
                    path: manifest.clone(),
                    field: key.into(),
                    value: value.to_string(),
                });
            }
        }
        directory.package = Some(Package { name, scripts });
    }
    let yaml_path = path.join("pnpm-workspace.yaml");
    if let Some(text) = reader.read(&yaml_path, boundary, false)? {
        // Tags/anchors can change semantics or expand input. The bounded first importer
        // supports ordinary declarative YAML only, and diagnoses advanced YAML explicitly.
        reject_advanced_yaml(&text, &yaml_path)?;
        let yaml: PnpmWorkspace = serde_yaml_ng::from_str(&text)
            .map_err(|error| format!("Invalid {}: {error}", safe(yaml_path.display())))?;
        let patterns = yaml.packages.unwrap_or_default();
        if let Some(existing) = &directory.workspace
            && existing != &patterns
        {
            directory.problems.push(format!(
                "Conflicting npm/pnpm workspace declarations in {}",
                safe(path.display())
            ));
        }
        directory.evidence.push(Evidence {
            path: yaml_path.clone(),
            field: "packages".into(),
            value: serde_json::to_string(&patterns).expect("Strings serialize."),
        });
        directory.workspace = Some(patterns);
        if let Some(version) = yaml.node_version {
            directory.evidence.push(Evidence {
                path: yaml_path.clone(),
                field: "useNodeVersion".into(),
                value: version,
            });
        }
    }
    if directory.package.is_none() && directory.workspace.is_none() {
        return Ok(directory);
    }
    // A lockfile is only package-manager evidence, never proof of an installed tree.
    for (name, manager) in [
        ("npm-shrinkwrap.json", "npm"),
        ("package-lock.json", "npm"),
        ("pnpm-lock.yaml", "pnpm"),
        ("yarn.lock", "yarn"),
        ("bun.lock", "bun"),
        ("bun.lockb", "bun"),
    ] {
        if name == "bun.lockb" {
            // Binary Bun locks are unsupported; only identify their presence, never parse them.
            if std::fs::symlink_metadata(path.join(name)).is_ok() {
                directory.problems.push(format!(
                    "Unsupported Bun lockfile in {}",
                    safe(path.display())
                ));
                directory.managers.push(Manager {
                    name: manager.into(),
                    version: None,
                    source: path.join(name),
                    field: "lockfile".into(),
                });
            }
            continue;
        }
        if reader.read(&path.join(name), boundary, true)?.is_some() {
            directory.managers.push(Manager {
                name: manager.into(),
                version: None,
                source: path.join(name),
                field: "lockfile".into(),
            });
            if manager == "yarn" || manager == "bun" {
                directory.problems.push(format!(
                    "Unsupported package manager {manager} declared by {}",
                    safe(path.join(name).display())
                ));
            }
        }
    }
    for name in [".nvmrc", ".node-version", ".tool-versions"] {
        if let Some(text) = reader.read(&path.join(name), boundary, false)? {
            let value = if name == ".tool-versions" {
                text.lines()
                    .filter(|line| line.split_whitespace().next() == Some("nodejs"))
                    .collect::<Vec<_>>()
                    .join("; ")
            } else {
                text.trim().into()
            };
            if !value.is_empty() {
                directory.evidence.push(Evidence {
                    path: path.join(name),
                    field: "Node version declaration (not checked)".into(),
                    value,
                });
            }
        }
    }
    Ok(directory)
}

fn parse_manager(text: &str, source: &Path, field: &str) -> Result<Manager, String> {
    let invalid = || {
        format!(
            "Unsupported package manager declaration {text:?} in {} ({field}); expected npm@10/11.x.y or pnpm@10.x.y without URL/integrity suffix",
            safe(source.display())
        )
    };
    let (name, raw_version) = text.split_once('@').ok_or_else(invalid)?;
    let version = semver::Version::parse(raw_version).map_err(|_| invalid())?;
    if !version.pre.is_empty()
        || !version.build.is_empty()
        || !matches!((name, version.major), ("npm", 10 | 11) | ("pnpm", 10))
    {
        return Err(invalid());
    }
    Ok(Manager {
        name: name.into(),
        version: Some(version.to_string()),
        source: source.into(),
        field: field.into(),
    })
}

/// Reject YAML aliases, anchors and tags outside quoted scalars/comments.
pub(crate) fn reject_advanced_yaml(text: &str, source: &Path) -> Result<(), String> {
    for line in text.lines() {
        let mut single = false;
        let mut double = false;
        let mut escaped = false;
        let mut previous = ' ';
        for c in line.chars() {
            if double && escaped {
                escaped = false;
                continue;
            }
            if double && c == '\\' {
                escaped = true;
                continue;
            }
            if c == '\'' && !double {
                single = !single;
            }
            if c == '"' && !single {
                double = !double;
            }
            if !single && !double {
                if c == '#' && previous.is_whitespace() {
                    break;
                }
                if matches!(c, '&' | '*' | '!')
                    && (previous.is_whitespace() || "[{,:".contains(previous))
                {
                    return Err(format!(
                        "Unsupported YAML tag/anchor/alias in {}; quote workspace patterns and use ordinary declarative YAML",
                        safe(source.display())
                    ));
                }
            }
            previous = c;
        }
    }
    Ok(())
}
