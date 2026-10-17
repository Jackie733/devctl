use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use serde_json::Value;

use super::{Check, Managed, State};
use crate::discovery::{self, safe};

pub(super) struct Facts {
    pub checks: Vec<Check>,
    pub fingerprint: String,
}

pub(super) fn inspect(managed: &Managed, script: &str) -> Result<Facts, String> {
    let report = discovery::scan(&managed.dependency_root)?;
    inspect_report(managed, script, &report)
}

pub(super) fn inspect_report(
    managed: &Managed,
    script: &str,
    report: &discovery::Report,
) -> Result<Facts, String> {
    if !report.problems.is_empty() {
        return Err(report.problems.join("; "));
    }
    let candidate = report
        .candidates
        .iter()
        .find(|candidate| {
            candidate.path == managed.project
                && candidate.dependency_root == managed.dependency_root
        })
        .ok_or("Configured project is not a declared member of its dependency root.")?;
    if !candidate
        .scripts
        .get(script)
        .is_some_and(|command| !command.trim().is_empty())
    {
        return Err(format!(
            "Selected script {} is absent or empty in {}",
            safe(script),
            safe(managed.project.display())
        ));
    }
    let mut checks = Vec::new();
    let mut dependency_checks = Vec::new();
    let mut sources = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut declared_node = false;
    for member in &report.candidates {
        if member.dependency_root != managed.dependency_root {
            continue;
        }
        sources.insert(member.path.join("package.json"));
        dependency_checks.push((
            member.path.clone(),
            dependencies(&member.path, &managed.dependency_root)?,
        ));
        sources.insert(member.path.join(".npmrc"));
        for evidence in &member.evidence {
            sources.insert(evidence.path.clone());
        }
        if let Some(manager) = &member.manager
            && (manager.name != managed.environment.package_manager
                || manager
                    .version
                    .as_ref()
                    .is_some_and(|version| version != &managed.environment.package_manager_version))
        {
            checks.push(Check::new(
                "package manager declaration",
                State::Incompatible,
                format!(
                    "{} requires {} {}; selected {} {}",
                    safe(member.path.display()),
                    manager.name,
                    manager
                        .version
                        .as_deref()
                        .unwrap_or("(version unspecified)"),
                    managed.environment.package_manager,
                    managed.environment.package_manager_version
                ),
            ));
        }
        for evidence in &member.evidence {
            if !seen.insert((
                evidence.path.clone(),
                evidence.field.clone(),
                evidence.value.clone(),
            )) {
                continue;
            }
            let source = format!(
                "{} ({})",
                safe(evidence.path.display()),
                safe(&evidence.field)
            );
            match evidence.field.as_str() {
                "engines" | "volta" => {
                    let value: Value = serde_json::from_str(&evidence.value)
                        .map_err(|_| format!("Invalid version evidence in {source}"))?;
                    let Some(object) = value.as_object() else {
                        checks.push(Check::new("version declaration", State::Unknown, source));
                        continue;
                    };
                    if let Some(node) = object.get("node") {
                        declared_node = true;
                        checks.push(requirement(
                            "Node requirement",
                            node,
                            &managed.environment.version,
                            &source,
                        ));
                    }
                    for name in ["npm", "pnpm"] {
                        if let Some(version) = object.get(name) {
                            if name == managed.environment.package_manager {
                                checks.push(requirement(
                                    "package manager requirement",
                                    version,
                                    &managed.environment.package_manager_version,
                                    &source,
                                ));
                            } else if evidence.field == "volta" {
                                checks.push(Check::new("tool declaration", State::Unknown, format!("Additional {name} tool pin in {source}; cross-manager toolchains are unsupported.")));
                            }
                        }
                    }
                }
                "devEngines" => {
                    let value: Value = serde_json::from_str(&evidence.value)
                        .map_err(|_| format!("Invalid devEngines in {source}"))?;
                    if !value.is_object() {
                        checks.push(Check::new("devEngines", State::Unknown, source));
                        continue;
                    }
                    for (key, expected, selected) in [
                        ("runtime", "node", managed.environment.version.as_str()),
                        (
                            "packageManager",
                            managed.environment.package_manager.as_str(),
                            managed.environment.package_manager_version.as_str(),
                        ),
                    ] {
                        if let Some(declaration) = value.get(key) {
                            let policy = declaration
                                .get("onFail")
                                .and_then(Value::as_str)
                                .unwrap_or("error");
                            if matches!(policy, "warn" | "ignore") {
                                checks.push(Check::new("advisory devEngines", State::Satisfied, format!("{source}: {key} is advisory ({policy}), not a hard compatibility claim.")));
                                continue;
                            }
                            if policy != "error"
                                || !declaration.is_object()
                                || declaration.get("name").and_then(Value::as_str) != Some(expected)
                            {
                                checks.push(Check::new(
                                    "devEngines",
                                    State::Unknown,
                                    format!(
                                        "Unsupported {key} alternatives/name/policy in {source}"
                                    ),
                                ));
                                continue;
                            }
                            if key == "runtime" {
                                declared_node = true;
                            }
                            if let Some(version) = declaration.get("version") {
                                checks.push(requirement(
                                    "devEngines requirement",
                                    version,
                                    selected,
                                    &source,
                                ));
                            }
                        }
                    }
                    for key in ["cpu", "os", "libc"] {
                        if value.get(key).is_some() {
                            checks.push(Check::new(
                                "devEngines platform",
                                State::Unknown,
                                format!("{source}: {key} constraints need manual verification."),
                            ));
                        }
                    }
                }
                "executionEnv" | "useNodeVersion" => checks.push(Check::new(
                    "runtime switching",
                    State::Unknown,
                    format!(
                        "{source} can activate another Node runtime; managed execution is blocked."
                    ),
                )),
                field if field.starts_with("Node version declaration") => {
                    declared_node = true;
                    let raw = if evidence
                        .path
                        .file_name()
                        .is_some_and(|name| name == ".tool-versions")
                    {
                        let versions: Vec<_> = evidence.value.split_whitespace().collect();
                        if versions.len() != 2 {
                            checks.push(Check::new(
                                "Node version declaration",
                                State::Unknown,
                                source,
                            ));
                            continue;
                        }
                        versions[1]
                    } else {
                        evidence.value.trim()
                    };
                    let raw = raw.strip_prefix('v').unwrap_or(raw);
                    checks.push(requirement(
                        "Node version declaration",
                        &Value::String(raw.into()),
                        &managed.environment.version,
                        &source,
                    ));
                }
                _ => {}
            }
        }
        project_settings(
            &member.path.join(".npmrc"),
            &managed.dependency_root,
            &mut checks,
        )?;
    }
    if !declared_node {
        checks.push(Check::new("Node selection", State::Satisfied, format!("Explicitly selected {}; no additional repository Node constraint was declared. This does not prove application compatibility.", managed.environment.version)));
    }
    project_settings(
        &managed.dependency_root.join(".npmrc"),
        &managed.dependency_root,
        &mut checks,
    )?;
    sources.insert(managed.dependency_root.join(".npmrc"));
    let yaml = managed.dependency_root.join("pnpm-workspace.yaml");
    sources.insert(yaml.clone());
    if yaml.exists() {
        let text = super::tools::read_text(&yaml, 1024 * 1024)?;
        discovery::reject_advanced_yaml(&text, &yaml)?;
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text)
            .map_err(|_| "Cannot parse package-manager settings.".to_owned())?;
        if let Some(map) = value.as_mapping() {
            for key in map.keys().filter_map(serde_yaml_ng::Value::as_str) {
                if dangerous_setting(key) {
                    checks.push(Check::new(
                        "package manager settings",
                        State::Unknown,
                        format!(
                            "Unsupported executable/runtime configuration key {} in {}",
                            safe(key),
                            safe(yaml.display())
                        ),
                    ));
                }
            }
        }
    }
    let (path, mut dependency_check) = dependency_checks
        .into_iter()
        .min_by_key(|(_, check)| match check.state {
            State::Missing => 0,
            State::Unknown => 1,
            State::Incompatible => 2,
            State::Satisfied => 3,
        })
        .ok_or("Dependency root has no participating packages.")?;
    dependency_check.detail = format!(
        "{} (shared-root package {}).",
        dependency_check.detail,
        safe(path.display())
    );
    checks.push(dependency_check);
    // Bind the preview to complete declarations, not only their current conclusions.
    let mut evidence = Vec::new();
    let mut total = 0;
    for source in sources {
        evidence.push((
            source.clone(),
            match std::fs::symlink_metadata(&source) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(format!("Cannot inspect reviewed declarations: {error}")),
                Ok(_) => {
                    let resolved = source.canonicalize().map_err(|error| error.to_string())?;
                    if !resolved.starts_with(&report.boundary) {
                        return Err("Reviewed declaration escapes the repository boundary.".into());
                    }
                    let text = super::tools::read_text(&source, 16 * 1024 * 1024)?;
                    total += text.len();
                    if total > 64 * 1024 * 1024 {
                        return Err("Reviewed declarations exceed 64 MiB.".into());
                    }
                    Some(super::tools::digest(text.as_bytes()))
                }
            },
        ));
    }
    let fingerprint =
        super::tools::digest(&serde_json::to_vec(&evidence).map_err(|error| error.to_string())?);
    Ok(Facts {
        checks,
        fingerprint,
    })
}

fn project_settings(path: &Path, root: &Path, checks: &mut Vec<Check>) -> Result<(), String> {
    if std::fs::symlink_metadata(path).is_ok()
        && !path
            .canonicalize()
            .map_err(|error| error.to_string())?
            .starts_with(root)
    {
        return Err("Project tool settings escape the dependency root.".into());
    }
    check_settings(path, checks)
}

fn requirement(item: &str, declaration: &Value, version: &str, source: &str) -> Check {
    let Some(raw) = declaration.as_str() else {
        return Check::new(
            item,
            State::Unknown,
            format!("Non-string version in {source}"),
        );
    };
    let Ok(range) = raw.parse::<node_semver::Range>() else {
        return Check::new(
            item,
            State::Unknown,
            format!("Unsupported range {} in {source}", safe(raw)),
        );
    };
    let selected = version
        .parse::<node_semver::Version>()
        .expect("Configuration validates exact versions.");
    let state = if selected.satisfies(&range) {
        State::Satisfied
    } else {
        State::Incompatible
    };
    Check::new(
        item,
        state,
        format!("selected {version}; declared {} in {source}", safe(raw)),
    )
}

fn normalized(key: &str) -> String {
    key.chars()
        .filter(|c| *c != '-' && *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

fn dangerous_setting(key: &str) -> bool {
    matches!(
        normalized(key).as_str(),
        "scriptshell"
            | "shellemulator"
            | "usenodeversion"
            | "nodeversion"
            | "executionenv"
            | "configdependencies"
            | "pnpmfile"
            | "globalpnpmfile"
            | "onloadscript"
            | "onlybuiltdependenciesfile"
    )
}

fn check_settings(path: &Path, checks: &mut Vec<Check>) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "Cannot inspect tool settings {}: {error}",
                safe(path.display())
            ));
        }
        Ok(_) => {}
    }
    let text = super::tools::read_text(path, 1024 * 1024)?;
    for line in text
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with(['#', ';']))
    {
        let raw = line.split('=').next().unwrap_or("").trim();
        // npm's ini format decodes quoted JSON keys and array suffixes as well.
        let raw = raw
            .strip_prefix('\'')
            .and_then(|key| key.strip_suffix('\''))
            .unwrap_or(raw);
        let key: String = serde_json::from_str(raw)
            .unwrap_or_else(|_| raw.split([';', '#']).next().unwrap_or("").trim().into());
        let key = key.strip_suffix("[]").unwrap_or(&key);
        if dangerous_setting(key) {
            checks.push(Check::new(
                "tool settings",
                State::Unknown,
                format!(
                    "Unsupported runtime/hook key {} in {}; values were not displayed.",
                    safe(key),
                    safe(path.display())
                ),
            ));
        }
    }
    Ok(())
}

pub(super) fn external_settings(node: &Path) -> Result<(), String> {
    let mut paths = BTreeSet::new();
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        for relative in [".npmrc", ".config/pnpm/rc", "Library/Preferences/pnpm/rc"] {
            paths.insert(home.join(relative));
        }
    }
    if let Some(root) = node.parent().and_then(Path::parent) {
        paths.insert(root.join("etc/npmrc"));
    }
    for (key, value) in std::env::vars_os() {
        match normalized(&key.to_string_lossy()).as_str() {
            "npmconfiguserconfig" | "npmconfigglobalconfig" => {
                paths.insert(PathBuf::from(value));
            }
            "npmconfigprefix" => {
                paths.insert(PathBuf::from(value).join("etc/npmrc"));
            }
            _ => {}
        }
    }
    for (key, suffix) in [("XDG_CONFIG_HOME", "pnpm/rc"), ("PNPM_HOME", "rc")] {
        if let Some(path) = std::env::var_os(key) {
            paths.insert(PathBuf::from(path).join(suffix));
        }
    }
    let mut checks = Vec::new();
    for path in paths {
        check_settings(&path, &mut checks)?;
    }
    for (key, _) in std::env::vars_os() {
        let key = key.to_string_lossy();
        if let Some(setting) = key.to_ascii_lowercase().strip_prefix("npm_config_")
            && dangerous_setting(setting)
        {
            return Err(format!(
                "Unsupported runtime/hook environment setting {}",
                safe(key)
            ));
        }
    }
    for check in checks {
        check.require()?;
    }
    Ok(())
}

fn dependencies(project: &Path, root: &Path) -> Result<Check, String> {
    let value: Value = serde_json::from_str(&super::tools::read_text(
        &project.join("package.json"),
        1024 * 1024,
    )?)
    .map_err(|_| "Cannot parse project dependencies.".to_owned())?;
    let has_dependencies = [
        "dependencies",
        "devDependencies",
        "optionalDependencies",
        "peerDependencies",
    ]
    .into_iter()
    .any(|key| {
        value
            .get(key)
            .is_some_and(|value| value.as_object().is_none_or(|map| !map.is_empty()))
    });
    if !has_dependencies {
        return Ok(Check::new(
            "dependencies",
            State::Satisfied,
            "No dependencies declared by this package; application readiness has not been checked.",
        ));
    }
    let mut location = project;
    loop {
        if location.join("node_modules").is_dir() {
            return Ok(Check::new(
                "dependencies",
                State::Unknown,
                "Dependency directories exist, but no verified installation inventory/receipt is available. Run setup when installation support is available, or explicitly acknowledge unknown dependencies for this start.",
            ));
        }
        if location == root {
            break;
        }
        location = location
            .parent()
            .ok_or("Project is outside its dependency root.")?;
    }
    Ok(Check::new(
        "dependencies",
        State::Missing,
        "Declared dependencies have no installed node_modules tree. Dependency installation is not implemented yet.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    #[test]
    fn npm_range_conformance_covers_union_intersection_partial_and_prerelease_rules() {
        for (range, version, expected) in [
            ("^22.1.0 || >=24 <25", "24.0.0", State::Satisfied),
            (">=22 <24", "24.0.0", State::Incompatible),
            ("22", "22.8.1", State::Satisfied),
            ("22.x", "23.0.0", State::Incompatible),
            ("~22.1", "22.2.0", State::Incompatible),
            ("22.1.0 - 24.2", "24.2.9", State::Satisfied),
            ("^0.2.3", "0.3.0", State::Incompatible),
            ("*", "24.0.0-beta.1", State::Incompatible),
            (">=24.0.0-beta.1 <25", "24.0.0-beta.2", State::Satisfied),
            ("lts/*", "24.0.0", State::Unknown),
        ] {
            let check = requirement("test", &json!(range), version, "fixture");
            assert_eq!(check.state, expected, "range={range} version={version}");
        }
    }

    #[test]
    fn requirements_include_unselected_workspace_members_and_advisory_policies() {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = PathBuf::from("/tmp").join(format!(
            "devctl-env-facts-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("apps/web")).unwrap();
        fs::create_dir_all(root.join("apps/worker")).unwrap();
        fs::write(
            root.join("package.json"),
            json!({"workspaces":["apps/*"]}).to_string(),
        )
        .unwrap();
        fs::write(root.join("apps/web/package.json"), json!({"scripts":{"dev":"node server"},"devEngines":{"runtime":{"name":"node","version":"<20","onFail":"warn"}}}).to_string()).unwrap();
        fs::write(
            root.join("apps/worker/package.json"),
            json!({"engines":{"node":"<24"}}).to_string(),
        )
        .unwrap();
        let root = root.canonicalize().unwrap();
        let managed = Managed {
            environment: super::super::NodeEnvironment {
                version: "24.0.0".into(),
                package_manager: "npm".into(),
                package_manager_version: "11.0.0".into(),
            },
            project: root.join("apps/web"),
            dependency_root: root.clone(),
        };
        let facts = inspect(&managed, "dev").unwrap();
        assert!(
            facts
                .checks
                .iter()
                .any(|check| check.state == State::Incompatible && check.detail.contains("worker"))
        );
        assert!(facts.checks.iter().any(|check| check.item=="advisory devEngines" && check.state==State::Satisfied));
        fs::write(root.join("apps/worker/package.json"), "{}").unwrap();
        fs::write(
            root.join("package.json"),
            json!({"workspaces":["apps/*"],"devDependencies":{"example":"1.0.0"}}).to_string(),
        )
        .unwrap();
        assert!(
            inspect(&managed, "dev")
                .unwrap()
                .checks
                .iter()
                .any(|check| check.item == "dependencies" && check.state == State::Missing)
        );
        fs::create_dir(root.join("node_modules")).unwrap();
        assert!(
            inspect(&managed, "dev")
                .unwrap()
                .checks
                .iter()
                .any(|check| check.item == "dependencies" && check.state == State::Unknown)
        );
        fs::write(
            root.join(".npmrc"),
            "//registry.example/:_authToken=PRIVATE_TOKEN\nuse-node-version=22.0.0\n",
        )
        .unwrap();
        let facts = inspect(&managed, "dev").unwrap();
        assert!(
            facts
                .checks
                .iter()
                .any(|check| check.state == State::Unknown && check.item == "tool settings")
        );
        assert!(
            !facts
                .checks
                .iter()
                .any(|check| check.detail.contains("PRIVATE_TOKEN"))
        );
        fs::remove_dir_all(root).unwrap();
    }
}
