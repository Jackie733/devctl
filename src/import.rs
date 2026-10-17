//! Reviewed directory onboarding. Saving config never installs or starts anything.
mod storage;
mod terminal;

use std::{
    collections::{BTreeMap, BTreeSet},
    os::unix::ffi::OsStrExt,
    path::{Component, Path, PathBuf},
};

use sha2::{Digest, Sha256};
use toml_edit::{DocumentMut, Item, Table, TableLike, value};

use crate::{
    cli::ImportOptions,
    config::{self, Config},
    discovery::{self, Candidate, Report, safe},
    environment::{self, Managed, NodeEnvironment},
};

#[derive(Clone)]
struct Selection {
    path: PathBuf,
    project: String,
    service: String,
    script: String,
    environment: Option<NodeEnvironment>,
    replace: bool,
    existing: bool,
}

struct Plan {
    text: String,
    changes: Vec<String>,
    sources: BTreeMap<PathBuf, String>,
    config_fingerprint: String,
    service_ids: Vec<String>,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn report_fingerprint(report: &Report) -> String {
    digest(format!("{report:?}").as_bytes())
}
fn config_fingerprint(config: &Config) -> String {
    digest(format!("{config:?}").as_bytes())
}

fn relative<'a>(report: &'a Report, candidate: &'a Candidate) -> &'a Path {
    let path = candidate
        .path
        .strip_prefix(&report.root)
        .expect("discovery stays inside root");
    if path.as_os_str().is_empty() {
        Path::new(".")
    } else {
        path
    }
}

fn candidate_path(report: &Report, path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(
            "Project selections must be relative paths from the displayed directory, without '..'."
                .into(),
        );
    }
    let canonical = report
        .root
        .join(path)
        .canonicalize()
        .map_err(|_| format!("Unknown project selection {}", safe(path.display())))?;
    if !report
        .candidates
        .iter()
        .any(|candidate| candidate.path == canonical)
    {
        return Err(format!(
            "Project {} is not a displayed candidate.",
            safe(path.display())
        ));
    }
    Ok(canonical)
}

fn slug(text: &str) -> String {
    let mut output = String::new();
    for character in text.chars() {
        let character = if character.is_ascii_alphanumeric() {
            character.to_ascii_lowercase()
        } else {
            '-'
        };
        if character != '-' || (!output.is_empty() && !output.ends_with('-')) {
            output.push(character);
        }
        if output.len() >= 56 {
            break;
        }
    }
    let output = output.trim_matches('-');
    if output.is_empty() {
        "project".into()
    } else {
        output.into()
    }
}

fn unique(base: &str, identity: &[u8], used: &mut BTreeSet<String>) -> Result<String, String> {
    if used.insert(base.into()) {
        return Ok(base.into());
    }
    let hash = digest(identity);
    for length in [8, 16, 32, 64] {
        let id = format!("{base}-{}", &hash[..length]);
        if used.insert(id.clone()) {
            return Ok(id);
        }
    }
    Err("Cannot assign an unambiguous project/environment ID.".into())
}

fn node_hint(candidate: &Candidate) -> Option<String> {
    for evidence in &candidate.evidence {
        let raw = if evidence.field.starts_with("Node version declaration") {
            if evidence
                .path
                .file_name()
                .is_some_and(|name| name == ".tool-versions")
            {
                evidence.value.split_whitespace().nth(1).map(str::to_owned)
            } else {
                Some(evidence.value.trim().into())
            }
        } else if matches!(evidence.field.as_str(), "engines" | "volta") {
            serde_json::from_str::<serde_json::Value>(&evidence.value)
                .ok()
                .and_then(|value| {
                    value
                        .get("node")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                })
        } else {
            None
        };
        if let Some(raw) = raw {
            let raw = raw.strip_prefix('v').unwrap_or(&raw);
            if semver::Version::parse(raw).is_ok() {
                return Some(raw.into());
            }
        }
    }
    None
}

async fn selections(
    report: &Report,
    existing: Option<&Config>,
    options: &ImportOptions,
    observed_node: Option<&str>,
) -> Result<Vec<Selection>, String> {
    let interactive = terminal::available() && !options.yes;
    let mut selected = BTreeSet::new();
    for path in &options.projects {
        selected.insert(candidate_path(report, path)?);
    }
    if selected.is_empty() {
        let runnable: Vec<_> = report
            .candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| !candidate.scripts.is_empty())
            .map(|(index, _)| index)
            .collect();
        if runnable.len() == 1 {
            selected.insert(report.candidates[runnable[0]].path.clone());
        } else if interactive && !runnable.is_empty() {
            let labels = report
                .candidates
                .iter()
                .map(|candidate| {
                    format!(
                        "{}{}",
                        relative(report, candidate).display(),
                        if candidate.scripts.is_empty() {
                            " (library, no scripts)"
                        } else {
                            ""
                        }
                    )
                })
                .collect::<Vec<_>>();
            let enabled = report
                .candidates
                .iter()
                .map(|candidate| !candidate.scripts.is_empty())
                .collect::<Vec<_>>();
            for index in
                terminal::choose("Select projects to import", &labels, &enabled, true, &[]).await?
            {
                selected.insert(report.candidates[index].path.clone());
            }
        } else {
            return Err("Selection required: use --project RELATIVE_PATH for the displayed runnable projects. No configuration changed.".into());
        }
    }
    let mut scripts = BTreeMap::new();
    for assignment in &options.scripts {
        let (path, script) = assignment
            .split_once('=')
            .ok_or("Use --script RELATIVE_PATH=SCRIPT.")?;
        let path = candidate_path(report, Path::new(path))?;
        if !selected.contains(&path) {
            return Err("A --script refers to an unselected project.".into());
        }
        if scripts.insert(path, script.to_owned()).is_some() {
            return Err("Select one script per project.".into());
        }
    }
    let replacements: BTreeSet<_> = options.replace.iter().cloned().collect();
    if replacements.len() != options.replace.len() {
        return Err("Duplicate --replace IDs.".into());
    }
    for id in &replacements {
        let project = existing
            .and_then(|config| config.projects.get(id))
            .ok_or_else(|| format!("Unknown replacement project '{}'.", safe(id)))?;
        if !selected.contains(&project.path) {
            return Err(format!(
                "Replacement '{}' is not a selected discovered project.",
                safe(id)
            ));
        }
    }
    let mut used: BTreeSet<_> = existing
        .into_iter()
        .flat_map(|config| config.projects.keys().chain(config.services.keys()))
        .cloned()
        .collect();
    let manager_override = options
        .package_manager
        .as_deref()
        .map(|text| {
            let (name, version) = text
                .split_once('@')
                .ok_or("Use --package-manager npm@EXACT or pnpm@EXACT.")?;
            if !matches!(name, "npm" | "pnpm") {
                return Err("Only npm/pnpm environments can be imported.".to_owned());
            }
            Ok((name.to_owned(), version.to_owned()))
        })
        .transpose()?;
    if let Some(version) = &options.node_version {
        NodeEnvironment {
            version: version.clone(),
            package_manager: "npm".into(),
            package_manager_version: "10.0.0".into(),
        }
        .validate()?;
    }
    if let Some((name, version)) = &manager_override {
        NodeEnvironment {
            version: "24.0.0".into(),
            package_manager: name.clone(),
            package_manager_version: version.clone(),
        }
        .validate()?;
    }
    let mut output = Vec::new();
    for candidate in report
        .candidates
        .iter()
        .filter(|candidate| selected.contains(&candidate.path))
    {
        let matches: Vec<_> = existing
            .into_iter()
            .flat_map(|config| &config.projects)
            .filter(|(_, project)| project.path == candidate.path)
            .map(|(id, _)| id.clone())
            .collect();
        let replacement = matches
            .iter()
            .filter(|id| replacements.contains(*id))
            .collect::<Vec<_>>();
        if replacement.len() > 1 {
            return Err(
                "One canonical project cannot replace several existing entries in one import."
                    .into(),
            );
        }
        let project_id = if let Some(id) = replacement.first() {
            (*id).clone()
        } else if let Some(id) = matches.first() {
            id.clone()
        } else {
            let label = if relative(report, candidate) == Path::new(".") {
                candidate
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            } else {
                relative(report, candidate).to_string_lossy().into_owned()
            };
            unique(
                &slug(&label),
                candidate.path.as_os_str().as_bytes(),
                &mut used,
            )?
        };
        let replace = replacements.contains(&project_id);
        let services: Vec<_> = existing
            .into_iter()
            .flat_map(|config| &config.services)
            .filter(|(_, service)| service.project == project_id)
            .collect();
        if !matches.is_empty() && !replace {
            println!(
                "Preserve project {}: already configured at {}; existing IDs, commands, cwd and autostart are retained. Use --replace {} to review a replacement.",
                safe(&project_id),
                safe(candidate.path.display()),
                safe(&project_id)
            );
            if matches.len() > 1 {
                println!(
                    "  Canonical aliases already exist as {}; no duplicate will be added.",
                    safe(matches.join(", "))
                );
            }
            if let Some(script) = scripts.get(&candidate.path) {
                if !candidate.scripts.contains_key(script) {
                    return Err(format!("Unknown declared script {}.", safe(script)));
                }
                println!(
                    "  Requested script {} is a proposed drift only; the existing command is preserved.",
                    safe(script)
                );
            }
            output.push(Selection {
                path: candidate.path.clone(),
                project: project_id,
                service: services
                    .first()
                    .map_or_else(String::new, |(id, _)| (*id).clone()),
                script: String::new(),
                environment: None,
                replace: false,
                existing: true,
            });
            continue;
        }
        if services.len() > 1 {
            return Err(format!(
                "Project {} has multiple services; resolve its replacement scope manually before importing.",
                safe(&project_id)
            ));
        }
        let service_id = services
            .first()
            .map(|(id, _)| (*id).clone())
            .unwrap_or_else(|| {
                if existing.is_some_and(|config| config.services.contains_key(&project_id)) {
                    String::new()
                } else {
                    project_id.clone()
                }
            });
        let service_id = if service_id.is_empty() {
            unique(
                &format!("{project_id}-service"),
                candidate.path.as_os_str().as_bytes(),
                &mut used,
            )?
        } else {
            service_id
        };
        let script = if let Some(script) = scripts.get(&candidate.path) {
            script.clone()
        } else if candidate.suggestions.len() == 1 {
            candidate.suggestions[0].clone()
        } else if interactive && !candidate.scripts.is_empty() {
            let names: Vec<_> = candidate.scripts.keys().cloned().collect();
            let labels: Vec<_> = names
                .iter()
                .map(|name| format!("{name} = {}", candidate.scripts[name]))
                .collect();
            let chosen = terminal::choose(
                &format!(
                    "Choose one script for {}",
                    relative(report, candidate).display()
                ),
                &labels,
                &vec![true; names.len()],
                false,
                &[],
            )
            .await?;
            names[chosen[0]].clone()
        } else {
            return Err(format!(
                "Selection required: use --script {}=SCRIPT. No startup script was invented.",
                safe(relative(report, candidate).display())
            ));
        };
        if script.is_empty()
            || script.starts_with('-')
            || !candidate
                .scripts
                .get(&script)
                .is_some_and(|text| !text.trim().is_empty())
        {
            return Err(format!(
                "Selected script {} is absent, empty or cannot be passed safely to the manager.",
                safe(&script)
            ));
        }
        let prior_environment = services
            .first()
            .and_then(|(_, service)| service.managed.as_ref())
            .map(|managed| &managed.environment);
        let version = options
            .node_version
            .clone()
            .or_else(|| observed_node.map(str::to_owned))
            .or_else(|| node_hint(candidate))
            .or_else(|| prior_environment.map(|environment| environment.version.clone()));
        let version = match version { Some(version) => version, None if interactive => terminal::exact("Select exact stable Node version; automatic release selection is not available yet").await?, None => return Err("Node selection required: use --node-version EXACT or --node /absolute/path/to/node. No LTS version was guessed.".into()) };
        let mut manager = manager_override
            .clone()
            .or_else(|| {
                candidate.manager.as_ref().map(|manager| {
                    (
                        manager.name.clone(),
                        manager.version.clone().unwrap_or_default(),
                    )
                })
            })
            .or_else(|| {
                prior_environment.map(|environment| {
                    (
                        environment.package_manager.clone(),
                        environment.package_manager_version.clone(),
                    )
                })
            });
        if let Some((name, version)) = &mut manager {
            if !matches!(name.as_str(), "npm" | "pnpm") {
                return Err("Only npm/pnpm environments can be imported.".into());
            }
            if version.is_empty() {
                if let Some(prior) =
                    prior_environment.filter(|prior| prior.package_manager == *name)
                {
                    *version = prior.package_manager_version.clone();
                } else if let Some(node) = &options.node
                    && let Ok(local) = environment::import_manager(node, name)
                {
                    *version = local;
                }
            }
        }
        if manager
            .as_ref()
            .is_none_or(|(_, version)| version.is_empty())
            && interactive
        {
            let answer =
                terminal::exact("Select exact package manager as npm@VERSION or pnpm@VERSION")
                    .await?;
            let (name, version) = answer
                .split_once('@')
                .ok_or("Select npm@EXACT or pnpm@EXACT.")?;
            manager = Some((name.into(), version.into()));
        }
        let (name, manager_version) = manager.ok_or(
            "Package manager selection required: use --package-manager npm@EXACT or pnpm@EXACT.",
        )?;
        if manager_version.is_empty() {
            return Err("Package manager version is unresolved; use --package-manager npm@EXACT or pnpm@EXACT.".into());
        }
        let environment = NodeEnvironment {
            version,
            package_manager: name,
            package_manager_version: manager_version,
        };
        environment.validate()?;
        output.push(Selection {
            path: candidate.path.clone(),
            project: project_id,
            service: service_id,
            script,
            environment: Some(environment),
            replace,
            existing: !matches.is_empty(),
        });
    }
    Ok(output)
}

fn group<'a>(document: &'a mut DocumentMut, name: &str) -> Result<&'a mut dyn TableLike, String> {
    if !document.contains_key(name) {
        let mut table = Table::new();
        table.set_implicit(true);
        document[name] = Item::Table(table);
    }
    document
        .get_mut(name)
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| {
            format!("Unsupported table layout for {name}; the original file is unchanged.")
        })
}

fn field(table: &mut dyn TableLike, key: &str, mut item: Item) {
    if let Some(old) = table.get(key).and_then(Item::as_value)
        && let Some(new) = item.as_value_mut()
    {
        *new.decor_mut() = old.decor().clone();
    }
    table.insert(key, item);
}

fn command(manager: &str, script: &str) -> Item {
    let array: toml_edit::Array = [manager, "run", script].into_iter().collect();
    value(array)
}

fn text_path(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| "Configuration paths must be UTF-8.".into())
}

fn make_plan(
    target: &Path,
    original: &Option<storage::Snapshot>,
    report: &Report,
    existing: Option<&Config>,
    selections: &[Selection],
    node: Option<&Path>,
) -> Result<Plan, String> {
    let mut document = match original {
        Some(original) => original.text.parse::<DocumentMut>().map_err(|_| {
            "Cannot edit the existing TOML; run validate for private diagnostics.".to_owned()
        })?,
        None => DocumentMut::new(),
    };
    let roundtrip = document.to_string();
    if original.is_none() {
        document["version"] = value(1);
        let mut workspace = Table::new();
        workspace["name"] = value(
            report
                .root
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or("imported-workspace"),
        );
        document["workspace"] = Item::Table(workspace);
    }
    let mut changes = Vec::new();
    if original.is_none() {
        changes.push(format!(
            "+ workspace.name = {}; version = 1",
            safe(
                document["workspace"]["name"]
                    .as_str()
                    .expect("generated workspace name")
            )
        ));
    }
    let mut sources = BTreeMap::new();
    let mut service_ids = Vec::new();
    let mut roots = BTreeMap::new();
    let mut checked_units = BTreeMap::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    let mut environment_ids =
        existing.map_or_else(BTreeMap::new, |config| config.node_environments.clone());
    let mut used_environments: BTreeSet<_> = environment_ids.keys().cloned().collect();
    for selection in selections {
        if std::time::Instant::now() >= deadline {
            return Err(
                "Import planning exceeded its filesystem budget; select fewer projects.".into(),
            );
        }
        let Some(environment) = &selection.environment else {
            continue;
        };
        let candidate = report
            .candidates
            .iter()
            .find(|candidate| candidate.path == selection.path)
            .ok_or("A selected project disappeared; preview again.")?;
        for previous in selections
            .iter()
            .filter(|previous| previous.path != selection.path && previous.environment.is_some())
        {
            let other = report
                .candidates
                .iter()
                .find(|candidate| candidate.path == previous.path)
                .ok_or("Selected project disappeared.")?;
            if other.dependency_root != candidate.dependency_root
                && (other
                    .dependency_root
                    .starts_with(&candidate.dependency_root)
                    || candidate
                        .dependency_root
                        .starts_with(&other.dependency_root))
            {
                return Err("Selected installation roots overlap; select one non-overlapping installation unit.".into());
            }
        }
        if !roots.contains_key(&candidate.dependency_root) {
            roots.insert(
                candidate.dependency_root.clone(),
                discovery::scan(&candidate.dependency_root)?,
            );
        }
        let managed = Managed {
            environment: environment.clone(),
            project: candidate.path.clone(),
            dependency_root: candidate.dependency_root.clone(),
        };
        let root_report = &roots[&candidate.dependency_root];
        let unit = (
            candidate.dependency_root.clone(),
            serde_json::to_string(environment).map_err(|error| error.to_string())?,
        );
        if !checked_units.contains_key(&unit) {
            checked_units.insert(
                unit.clone(),
                environment::import_fingerprint(&managed, &selection.script, root_report, node)?,
            );
        } else if !root_report.candidates.iter().any(|member| {
            member.path == selection.path
                && member.dependency_root == candidate.dependency_root
                && member
                    .scripts
                    .get(&selection.script)
                    .is_some_and(|script| !script.trim().is_empty())
        }) {
            return Err(
                "Selected project/script no longer belongs to its declared installation unit."
                    .into(),
            );
        }
        sources.insert(selection.path.clone(), checked_units[&unit].clone());
        let environment_id = if let Some((id, _)) = environment_ids
            .iter()
            .find(|(_, known)| *known == environment)
        {
            id.clone()
        } else {
            let base = format!(
                "node-{}-{}-{}",
                environment.version,
                environment.package_manager,
                environment.package_manager_version
            );
            let id = unique(&base, base.as_bytes(), &mut used_environments)?;
            let mut table = Table::new();
            table["version"] = value(&environment.version);
            table["package_manager"] = value(&environment.package_manager);
            table["package_manager_version"] = value(&environment.package_manager_version);
            group(&mut document, "node_environments")?.insert(&id, Item::Table(table));
            environment_ids.insert(id.clone(), environment.clone());
            changes.push(format!(
                "+ node_environments.{} = Node {}, {} {}",
                safe(&id),
                environment.version,
                environment.package_manager,
                environment.package_manager_version
            ));
            id
        };
        if !selection.existing {
            let mut project = Table::new();
            project["path"] = value(text_path(&candidate.path)?);
            if candidate.dependency_root != candidate.path {
                project["dependency_root"] = value(text_path(&candidate.dependency_root)?);
            }
            group(&mut document, "projects")?.insert(&selection.project, Item::Table(project));
            changes.push(format!(
                "+ projects.{}: path={} dependency_root={}",
                safe(&selection.project),
                safe(candidate.path.display()),
                safe(candidate.dependency_root.display())
            ));
        } else if selection.replace {
            let project = &existing.expect("existing project").projects[&selection.project];
            let prior_root = project.dependency_root.as_ref().unwrap_or(&project.path);
            if prior_root != &candidate.dependency_root {
                let table = group(&mut document, "projects")?
                    .get_mut(&selection.project)
                    .and_then(Item::as_table_like_mut)
                    .ok_or("Cannot edit project table.")?;
                field(
                    table,
                    "dependency_root",
                    value(text_path(&candidate.dependency_root)?),
                );
                changes.push(format!(
                    "~ projects.{}.dependency_root: {} -> {}",
                    safe(&selection.project),
                    safe(prior_root.display()),
                    safe(candidate.dependency_root.display())
                ));
            }
        }
        let prior = existing.and_then(|config| config.services.get(&selection.service));
        if prior.is_none() {
            let mut service = Table::new();
            service["project"] = value(&selection.project);
            service["cmd"] = command(&environment.package_manager, &selection.script);
            service["node_environment"] = value(&environment_id);
            group(&mut document, "services")?.insert(&selection.service, Item::Table(service));
            changes.push(format!(
                "+ services.{}: project={} cmd={:?} node_environment={} autostart=true",
                safe(&selection.service),
                safe(&selection.project),
                [&environment.package_manager, "run", &selection.script],
                safe(&environment_id)
            ));
        } else if let Some(prior) = prior.filter(|_| selection.replace) {
            let table = group(&mut document, "services")?
                .get_mut(&selection.service)
                .and_then(Item::as_table_like_mut)
                .ok_or("Cannot edit service table.")?;
            let expected = vec![
                environment.package_manager.clone(),
                "run".into(),
                selection.script.clone(),
            ];
            if prior.cmd != expected {
                field(
                    table,
                    "cmd",
                    command(&environment.package_manager, &selection.script),
                );
                changes.push(format!(
                    "~ services.{}.cmd: {} -> {}",
                    safe(&selection.service),
                    safe(format!("{:?}", prior.cmd)),
                    safe(format!("{expected:?}"))
                ));
            }
            if prior.cwd.as_ref() != Some(&candidate.path) {
                field(table, "cwd", value(text_path(&candidate.path)?));
                changes.push(format!(
                    "~ services.{}.cwd: {} -> {}",
                    safe(&selection.service),
                    safe(prior.cwd.as_deref().unwrap_or(&candidate.path).display()),
                    safe(candidate.path.display())
                ));
            }
            if prior.node_environment.as_ref() != Some(&environment_id) {
                field(table, "node_environment", value(&environment_id));
                changes.push(format!(
                    "~ services.{}.node_environment: {} -> {}",
                    safe(&selection.service),
                    safe(prior.node_environment.as_deref().unwrap_or("legacy")),
                    safe(&environment_id)
                ));
            }
        }
        service_ids.push(selection.service.clone());
    }
    let text = if changes.is_empty() {
        original
            .as_ref()
            .map_or_else(|| document.to_string(), |original| original.text.clone())
    } else {
        if let Some(original) = original
            && roundtrip.trim_end_matches('\n') != original.text.trim_end_matches('\n')
        {
            return Err("This TOML layout cannot round-trip without unrelated formatting changes. Import refused; retain the original and normalize its formatting manually first.".into());
        }
        document.to_string()
    };
    let proposed = config::parse(&text, target).map_err(|_| "The complete proposed configuration is invalid. Original bytes are unchanged; run validate separately for private diagnostics.".to_owned())?;
    if std::time::Instant::now() >= deadline {
        return Err(
            "Import planning exceeded its filesystem budget; select fewer projects.".into(),
        );
    }
    Ok(Plan {
        text,
        changes,
        sources,
        config_fingerprint: config_fingerprint(&proposed),
        service_ids,
    })
}

pub async fn run(
    directory: &Path,
    config_path: Option<PathBuf>,
    options: &ImportOptions,
) -> Result<(), String> {
    let report = discovery::scan(directory)?;
    discovery::print_report(&report);
    if !report.problems.is_empty() {
        return Err("Project discovery has unresolved problems; no commands were executed or files changed.".into());
    }
    let requested = config::resolve_import_path(config_path)?;
    let target = storage::identity(&requested)?;
    text_path(&target)?;
    if let Some(node) = &options.node {
        text_path(node)?;
    }
    println!("Configuration destination: {}", safe(target.display()));
    let original = storage::read(&target)?;
    let existing = original.as_ref().map(|original| config::parse(&original.text, &target).map_err(|_| "Existing configuration is invalid; it will not be replaced. Run validate separately for private diagnostics.".to_owned())).transpose()?;
    let observed_node = match &options.node {
        Some(path) => Some(environment::import_node(path).await?),
        None => None,
    };
    if let (Some(expected), Some(actual)) = (&options.node_version, &observed_node)
        && expected != actual
    {
        return Err(format!(
            "Explicit Node selection disagrees: requested {}; observed {}. No configuration changed.",
            safe(expected),
            safe(actual)
        ));
    }
    let selections = selections(
        &report,
        existing.as_ref(),
        options,
        observed_node.as_deref(),
    )
    .await?;
    let plan = make_plan(
        &target,
        &original,
        &report,
        existing.as_ref(),
        &selections,
        options.node.as_deref(),
    )?;
    if let Some(existing) = &existing {
        for (id, project) in &existing.projects {
            if project.path.starts_with(&report.root)
                && !report
                    .candidates
                    .iter()
                    .any(|candidate| candidate.path == project.path)
            {
                println!(
                    "Retain existing project {}: no longer discovered here; no entry was deleted.",
                    safe(id)
                );
            }
        }
    }
    println!("\nConfiguration diff (only additions/explicit replacements):");
    for change in &plan.changes {
        println!("  {change}");
    }
    if plan.changes.is_empty() {
        println!("  No changes; existing configuration and edits are preserved.");
    }
    println!(
        "No tools/dependencies will be installed and no services will start. Older devctl versions reject managed Node fields; shut down this workspace before changing binaries."
    );
    if !plan.service_ids.is_empty() {
        let proposed = config::parse(&plan.text, &target)?;
        let _findings =
            environment::doctor(&proposed, &plan.service_ids, options.node.as_deref()).await;
        println!(
            "Environment findings describe current installations. Saving version choices does not establish application readiness."
        );
    }
    if !options.apply {
        return Ok(());
    }
    if !options.yes && !terminal::available() {
        return Err(
            "Non-interactive import apply requires --yes after reviewing the explicit selections."
                .into(),
        );
    }
    if !plan.changes.is_empty()
        && !options.yes
        && !environment::confirm("Save this configuration diff? [y/N] ").await?
    {
        return Err("Import cancelled; no configuration changed.".into());
    }
    let transaction = storage::Transaction::begin(&requested, &target, &original)?;
    let fresh = discovery::scan(directory)?;
    if report_fingerprint(&fresh) != report_fingerprint(&report) {
        return Err(
            "Project discovery changed after review; preview again. No configuration was written."
                .into(),
        );
    }
    let fresh_plan = make_plan(
        &target,
        &original,
        &fresh,
        existing.as_ref(),
        &selections,
        options.node.as_deref(),
    )?;
    if fresh_plan.sources != plan.sources
        || fresh_plan.text != plan.text
        || fresh_plan.config_fingerprint != plan.config_fingerprint
    {
        return Err("Reviewed source/configuration evidence changed; preview again. No configuration was written.".into());
    }
    if let Some(node) = &options.node
        && Some(environment::import_node(node).await?) != observed_node
    {
        return Err("Selected Node changed after review; preview again.".into());
    }
    if plan.changes.is_empty() {
        println!("No changes; configuration was not rewritten.");
        return Ok(());
    }
    transaction.publish(&plan.text)?;
    drop(transaction);
    println!("Saved configuration: {}", safe(target.display()));
    for selection in selections
        .iter()
        .filter(|selection| selection.environment.is_some())
    {
        println!(
            "  Project {} / service {}",
            safe(&selection.project),
            safe(&selection.service)
        );
    }
    let ids = plan
        .service_ids
        .iter()
        .map(|id| shell_word(id))
        .collect::<Vec<_>>()
        .join(" ");
    let config_argument = shell_word(text_path(&target)?);
    println!("Next: devctl --config {config_argument} doctor {ids}");
    let node_argument = options
        .node
        .as_ref()
        .map(|node| text_path(node).map(|node| format!(" --node {}", shell_word(node))))
        .transpose()?
        .unwrap_or_default();
    println!(
        "Then preview local binding: devctl --config {config_argument} setup {ids}{node_argument}"
    );
    println!("After environment checks: devctl --config {config_argument} tui");
    Ok(())
}

fn shell_word(text: &str) -> String {
    if !text.chars().any(char::is_control) {
        return format!("'{}'", text.replace('\'', "'\\''"));
    }
    // Bash/Zsh ANSI quoting keeps control characters out of terminal output.
    let mut output = String::from("$'");
    for character in text.chars() {
        match character {
            '\'' => output.push_str("\\'"),
            '\\' => output.push_str("\\\\"),
            character if character.is_control() => {
                let mut bytes = [0; 4];
                for byte in character.encode_utf8(&mut bytes).as_bytes() {
                    output.push_str(&format!("\\x{byte:02x}"));
                }
            }
            character => output.push(character),
        }
    }
    output.push('\'');
    output
}
