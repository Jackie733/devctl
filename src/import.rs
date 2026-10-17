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
};

#[derive(Clone)]
struct Selection {
    path: PathBuf,
    project: String,
    service: String,
    command: Option<Vec<String>>,
    replace: bool,
    existing: bool,
}

struct Plan {
    text: String,
    changes: Vec<String>,
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
    Err("Cannot assign an unambiguous project ID.".into())
}

async fn selections(
    report: &Report,
    existing: Option<&Config>,
    options: &ImportOptions,
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
                command: None,
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
        let manager = options.package_manager.as_deref().or_else(|| {
            candidate
                .manager
                .as_ref()
                .map(|manager| manager.name.as_str())
        });
        let manager = match manager {
            Some(manager) => manager.to_owned(),
            None if interactive => {
                let names = vec!["npm".to_owned(), "pnpm".to_owned()];
                let selected = terminal::choose("Choose package manager command (versions are not managed)", &names, &[true, true], false, &[]).await?;
                names[selected[0]].clone()
            }
            None => return Err("Package manager command is unknown; select --package-manager npm or pnpm, or use devctl add with an explicit command.".into()),
        };
        if !matches!(manager.as_str(), "npm" | "pnpm") {
            return Err(
                "Script import supports npm/pnpm; use devctl add for any other command.".into(),
            );
        }
        if candidate
            .manager
            .as_ref()
            .is_some_and(|declared| declared.name != manager)
        {
            return Err("Selected command conflicts with the declared package manager; use devctl add for an explicit custom command.".into());
        }
        output.push(Selection {
            path: candidate.path.clone(),
            project: project_id,
            service: service_id,
            command: Some(vec![manager, "run".into(), script]),
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

fn command(arguments: &[String]) -> Item {
    value(
        arguments
            .iter()
            .map(String::as_str)
            .collect::<toml_edit::Array>(),
    )
}

fn text_path(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| "Configuration paths must be UTF-8.".into())
}

fn make_plan(
    target: &Path,
    original: &Option<storage::Snapshot>,
    directory: &Path,
    existing: Option<&Config>,
    selections: &[Selection],
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
            directory
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
    let mut service_ids = Vec::new();
    for selection in selections {
        let Some(arguments) = &selection.command else {
            continue;
        };
        if !selection.existing {
            let mut project = Table::new();
            project["path"] = value(text_path(&selection.path)?);
            group(&mut document, "projects")?.insert(&selection.project, Item::Table(project));
            changes.push(format!(
                "+ projects.{}: path={}",
                safe(&selection.project),
                safe(selection.path.display())
            ));
        }
        let prior = existing.and_then(|config| config.services.get(&selection.service));
        if prior.is_none() {
            let mut service = Table::new();
            service["project"] = value(&selection.project);
            service["cmd"] = command(arguments);
            group(&mut document, "services")?.insert(&selection.service, Item::Table(service));
            changes.push(format!(
                "+ services.{}: project={} cmd={} autostart=true",
                safe(&selection.service),
                safe(&selection.project),
                safe(format!("{arguments:?}"))
            ));
        } else if let Some(prior) = prior.filter(|_| selection.replace) {
            let table = group(&mut document, "services")?
                .get_mut(&selection.service)
                .and_then(Item::as_table_like_mut)
                .ok_or("Cannot edit service table.")?;
            let expected = arguments;
            if &prior.cmd != expected {
                field(table, "cmd", command(arguments));
                changes.push(format!(
                    "~ services.{}.cmd: {} -> {}",
                    safe(&selection.service),
                    safe(format!("{:?}", prior.cmd)),
                    safe(format!("{expected:?}"))
                ));
            }
            if prior.cwd.as_ref() != Some(&selection.path) {
                field(table, "cwd", value(text_path(&selection.path)?));
                changes.push(format!(
                    "~ services.{}.cwd: {} -> {}",
                    safe(&selection.service),
                    safe(prior.cwd.as_deref().unwrap_or(&selection.path).display()),
                    safe(selection.path.display())
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
    if text.len() > 1024 * 1024 {
        return Err("Proposed configuration exceeds 1 MiB.".into());
    }
    let proposed = config::parse(&text, target).map_err(|_| "The complete proposed configuration is invalid. Original bytes are unchanged; run validate separately for private diagnostics.".to_owned())?;
    Ok(Plan {
        text,
        changes,
        config_fingerprint: config_fingerprint(&proposed),
        service_ids,
    })
}

struct Destination {
    requested: PathBuf,
    target: PathBuf,
    original: Option<storage::Snapshot>,
    existing: Option<Config>,
}

impl Destination {
    fn open(path: Option<PathBuf>) -> Result<Self, String> {
        let requested = config::resolve_import_path(path)?;
        let target = storage::identity(&requested)?;
        text_path(&target)?;
        println!("Configuration destination: {}", safe(target.display()));
        let original = storage::read(&target)?;
        let existing = original.as_ref().map(|original| config::parse(&original.text, &target).map_err(|_| "Existing configuration is invalid; it will not be replaced. Run validate separately for private diagnostics.".to_owned())).transpose()?;
        Ok(Self {
            requested,
            target,
            original,
            existing,
        })
    }

    fn plan(&self, directory: &Path, selections: &[Selection]) -> Result<Plan, String> {
        make_plan(
            &self.target,
            &self.original,
            directory,
            self.existing.as_ref(),
            selections,
        )
    }
}

async fn save(
    destination: &Destination,
    plan: &Plan,
    apply: bool,
    yes: bool,
    recheck: impl FnOnce() -> Result<Plan, String>,
) -> Result<(), String> {
    println!("\nConfiguration diff (only additions/explicit replacements):");
    for change in &plan.changes {
        println!("  {change}");
    }
    if plan.changes.is_empty() {
        println!("  No changes; existing configuration and edits are preserved.");
    }
    println!(
        "Commands use the existing environment. No tools are checked or installed; no services will start."
    );
    if !apply {
        return Ok(());
    }
    if !yes && !terminal::available() {
        return Err(
            "Non-interactive import apply requires --yes after reviewing the explicit selections."
                .into(),
        );
    }
    if !plan.changes.is_empty()
        && !yes
        && !terminal::confirm("Save this configuration diff? [y/N] ").await?
    {
        return Err("Import cancelled; no configuration changed.".into());
    }
    let transaction = storage::Transaction::begin(
        &destination.requested,
        &destination.target,
        &destination.original,
    )?;
    let fresh = recheck()?;
    if fresh.text != plan.text || fresh.config_fingerprint != plan.config_fingerprint {
        return Err("Reviewed source/configuration evidence changed; preview again. No configuration was written.".into());
    }
    if plan.changes.is_empty() {
        println!("No changes; configuration was not rewritten.");
        return Ok(());
    }
    transaction.publish(&plan.text)?;
    drop(transaction);
    println!(
        "Saved configuration: {}",
        safe(destination.target.display())
    );
    let config_argument = shell_word(text_path(&destination.target)?);
    let ids = plan
        .service_ids
        .iter()
        .map(|id| shell_word(id))
        .collect::<Vec<_>>()
        .join(" ");
    println!("Next: devctl --config {config_argument} tui");
    println!("Or start: devctl --config {config_argument} start {ids}");
    Ok(())
}

pub async fn run(
    directory: &Path,
    config_path: Option<PathBuf>,
    options: &ImportOptions,
) -> Result<(), String> {
    let report = discovery::scan(directory)?;
    discovery::print_report(&report);
    if !report.problems.is_empty() {
        return Err("Project discovery has unresolved problems; use devctl add with an explicit directory and command. No files changed.".into());
    }
    let destination = Destination::open(config_path)?;
    let selections = selections(&report, destination.existing.as_ref(), options).await?;
    let plan = destination.plan(&report.root, &selections)?;
    if let Some(existing) = &destination.existing {
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
    save(&destination, &plan, options.apply, options.yes, || {
        let fresh = discovery::scan(directory)?;
        if report_fingerprint(&fresh) != report_fingerprint(&report) {
            return Err("Reviewed source evidence changed after review; preview again. No configuration was written.".into());
        }
        destination.plan(&fresh.root, &selections)
    }).await
}

/// Language-independent addition: directory and argv are supplied by the user.
pub async fn add(
    directory: &Path,
    config_path: Option<PathBuf>,
    name: Option<&str>,
    arguments: &[String],
    apply: bool,
    yes: bool,
) -> Result<(), String> {
    let path = directory.canonicalize().map_err(|error| {
        format!(
            "Cannot resolve project directory {}: {error}",
            safe(directory.display())
        )
    })?;
    if !path.is_dir() {
        return Err("Project path must be a directory.".into());
    }
    if arguments
        .first()
        .is_none_or(|argument| argument.trim().is_empty())
    {
        return Err("Provide a non-empty command after --.".into());
    }
    let destination = Destination::open(config_path)?;
    let existing_project = destination.existing.as_ref().and_then(|config| {
        config
            .projects
            .iter()
            .find(|(_, project)| project.path == path)
    });
    let mut used: BTreeSet<_> = destination
        .existing
        .iter()
        .flat_map(|config| config.projects.keys().chain(config.services.keys()))
        .cloned()
        .collect();
    let (id, existing) = if let Some((id, _)) = existing_project {
        println!(
            "Preserve project {}: already configured; existing commands and edits are retained. Edit the configuration to change an explicit command.",
            safe(id)
        );
        (id.clone(), true)
    } else if let Some(name) = name {
        if name.trim().is_empty() {
            return Err("Project name must not be empty.".into());
        }
        if !used.insert(name.into()) {
            return Err("Project/service name already exists; choose another --name. No entries were overwritten.".into());
        }
        (name.to_owned(), false)
    } else {
        (
            unique(
                &slug(
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .as_ref(),
                ),
                path.as_os_str().as_bytes(),
                &mut used,
            )?,
            false,
        )
    };
    let selections = vec![Selection {
        path: path.clone(),
        project: id.clone(),
        service: id,
        command: (!existing).then(|| arguments.to_vec()),
        replace: false,
        existing,
    }];
    let plan = destination.plan(&path, &selections)?;
    save(&destination, &plan, apply, yes, || {
        if directory.canonicalize().ok().as_ref() != Some(&path) || !path.is_dir() {
            return Err("Project directory changed after review; preview again.".into());
        }
        destination.plan(&path, &selections)
    })
    .await
}

pub async fn interruptible(
    future: impl std::future::Future<Output = Result<(), String>>,
) -> Result<(), String> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut interrupt = signal(SignalKind::interrupt()).map_err(|error| error.to_string())?;
    let mut terminate = signal(SignalKind::terminate()).map_err(|error| error.to_string())?;
    tokio::select! {
        result = future => result,
        _ = interrupt.recv() => Err("Configuration operation cancelled.".into()),
        _ = terminate.recv() => Err("Configuration operation cancelled.".into()),
    }
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
