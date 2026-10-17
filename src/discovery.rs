//! Bounded, read-only repository discovery. No executable is launched here.

mod files;
mod manifest;
pub(crate) use manifest::reject_advanced_yaml;

#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use globset::{GlobBuilder, GlobMatcher};

use files::{Limits, Reader};
use manifest::{Directory, Evidence};

#[derive(Debug)]
pub(crate) struct Candidate {
    pub path: PathBuf,
    pub dependency_root: PathBuf,
    pub name: Option<String>,
    pub scripts: BTreeMap<String, String>,
    pub suggestions: Vec<String>,
    pub manager: Option<Manager>,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Manager {
    pub name: String,
    pub version: Option<String>,
    pub source: PathBuf,
    pub field: String,
}

#[derive(Debug)]
pub(crate) struct Report {
    pub root: PathBuf,
    pub boundary: PathBuf,
    pub declaration_root: PathBuf,
    pub candidates: Vec<Candidate>,
    pub warnings: Vec<String>,
    pub problems: Vec<String>,
}

struct Workspace {
    path: PathBuf,
    include: Vec<GlobMatcher>,
    exclude: Vec<GlobMatcher>,
    branches: Vec<Vec<Segment>>,
}

enum Segment {
    Recursive,
    Match(GlobMatcher),
}

impl Workspace {
    fn new(path: &Path, patterns: &[String]) -> Result<Self, String> {
        let mut workspace = Self {
            path: path.into(),
            include: Vec::new(),
            exclude: Vec::new(),
            branches: Vec::new(),
        };
        if patterns.len() > 200 {
            return Err(format!(
                "Discovery incomplete: workspace pattern count limit reached in {}",
                safe(path.display())
            ));
        }
        for raw in patterns {
            let (negative, pattern) = raw
                .strip_prefix('!')
                .map_or((false, raw.as_str()), |pattern| (true, pattern));
            let pattern = pattern
                .strip_prefix("./")
                .unwrap_or(pattern)
                .trim_end_matches('/');
            if pattern.is_empty()
                || pattern.len() > 4096
                || pattern.split('/').count() > 32
                || pattern.starts_with('/')
                || pattern.split('/').any(|part| part == "..")
                || pattern.chars().any(|c| "[]{}\\!".contains(c))
                || pattern
                    .split('/')
                    .any(|part| part.contains("**") && part != "**")
            {
                return Err(format!(
                    "Unsupported workspace pattern {raw:?} in {}",
                    path.display()
                ));
            }
            let matcher = GlobBuilder::new(pattern)
                .literal_separator(true)
                .backslash_escape(false)
                .build()
                .map_err(|error| format!("Invalid workspace pattern {raw:?}: {error}"))?
                .compile_matcher();
            if negative {
                workspace.exclude.push(matcher);
                // A subtree exclusion also excludes its directory itself.
                if let Some(base) = pattern.strip_suffix("/**") {
                    workspace.exclude.push(
                        GlobBuilder::new(base)
                            .literal_separator(true)
                            .build()
                            .map_err(|error| error.to_string())?
                            .compile_matcher(),
                    );
                }
            } else {
                workspace.include.push(matcher);
                let segments = pattern
                    .split('/')
                    .map(|part| {
                        if part == "**" {
                            Ok(Segment::Recursive)
                        } else {
                            GlobBuilder::new(part)
                                .literal_separator(true)
                                .build()
                                .map(|glob| Segment::Match(glob.compile_matcher()))
                                .map_err(|error| error.to_string())
                        }
                    })
                    .collect::<Result<_, _>>()?;
                workspace.branches.push(segments);
            }
        }
        Ok(workspace)
    }

    fn contains(&self, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(&self.path) else {
            return false;
        };
        !relative.as_os_str().is_empty()
            && self
                .include
                .iter()
                .any(|pattern| pattern.is_match(relative))
            && !self.excluded(relative)
    }

    fn excluded(&self, relative: &Path) -> bool {
        relative.ancestors().any(|ancestor| {
            self.exclude
                .iter()
                .any(|pattern| pattern.is_match(ancestor))
        })
    }

    /// Match a path prefix, so declared workspaces need not walk unrelated trees.
    /// Iterative glob states keep ** matching bounded without recursive backtracking.
    fn reachable(&self, path: &Path, descendants: bool) -> bool {
        let Ok(relative) = path.strip_prefix(&self.path) else {
            return false;
        };
        if self.excluded(relative) {
            return false;
        }
        self.branches.iter().any(|segments| {
            let mut states = vec![false; segments.len() + 1];
            states[0] = true;
            advance_recursive(segments, &mut states);
            for component in relative.components() {
                let mut next = vec![false; states.len()];
                for (index, segment) in segments.iter().enumerate() {
                    if !states[index] {
                        continue;
                    }
                    match segment {
                        Segment::Recursive => next[index] = true,
                        Segment::Match(pattern)
                            if pattern.is_match(Path::new(component.as_os_str())) =>
                        {
                            next[index + 1] = true
                        }
                        Segment::Match(_) => {}
                    }
                }
                advance_recursive(segments, &mut next);
                states = next;
            }
            states[..states.len() - usize::from(descendants)]
                .iter()
                .any(|state| *state)
        })
    }
}

fn advance_recursive(segments: &[Segment], states: &mut [bool]) {
    for (index, segment) in segments.iter().enumerate() {
        if states[index] && matches!(segment, Segment::Recursive) {
            states[index + 1] = true;
        }
    }
}

pub(crate) fn scan(path: &Path) -> Result<Report, String> {
    scan_with_limits(path, Limits::default())
}

fn scan_with_limits(path: &Path, limits: Limits) -> Result<Report, String> {
    let root = path.canonicalize().map_err(|error| {
        format!(
            "Cannot resolve project directory {}: {error}",
            safe(path.display())
        )
    })?;
    if !root.is_dir() {
        return Err(format!(
            "Project path is not a directory: {}",
            safe(root.display())
        ));
    }
    let mut reader = Reader::new(limits);
    let ancestors = reader.ancestors(&root)?;
    let boundary = ancestors
        .last()
        .expect("The supplied directory is an ancestor.")
        .clone();
    let git_boundary = std::fs::symlink_metadata(boundary.join(".git")).is_ok();
    let mut report = Report {
        root: root.clone(),
        boundary: boundary.clone(),
        declaration_root: if git_boundary {
            boundary.clone()
        } else {
            root.clone()
        },
        candidates: Vec::new(),
        warnings: Vec::new(),
        problems: Vec::new(),
    };
    let mut directories = BTreeMap::new();
    let mut workspaces = Vec::new();
    // Parents are inspected for declared workspace membership, never searched recursively.
    for ancestor in ancestors.iter().rev().filter(|path| *path != &root) {
        // Without Git, a parent declaration must stay inside that parent directory.
        let source_boundary = if git_boundary { &boundary } else { ancestor };
        match reader.directory(ancestor, source_boundary) {
            Ok(directory) => {
                if directory.workspace.is_some() {
                    add_workspace(&directory, &mut workspaces, &mut report);
                }
                directories.insert(ancestor.clone(), directory);
            }
            Err(error) => report.problems.push(error),
        }
    }
    if !git_boundary
        && let Some(workspace) = workspaces
            .iter()
            .find(|workspace| workspace.reachable(&root, false))
    {
        report.declaration_root = workspace.path.clone();
    }
    let result = visit(
        &root,
        0,
        &mut reader,
        &mut directories,
        &mut workspaces,
        &mut report,
    );
    if let Err(error) = result {
        report.problems.push(error);
    }
    // All directory reads finish before resolving inheritance or presenting commands.
    for (path, directory) in &directories {
        if !path.starts_with(&root) {
            continue;
        }
        let Some(package) = &directory.package else {
            continue;
        };
        let own_workspace = workspaces.iter().find(|workspace| workspace.path == *path);
        let parent_workspace = workspaces
            .iter()
            .filter(|workspace| workspace.contains(path))
            .max_by_key(|workspace| workspace.path.components().count());
        let dependency_root = own_workspace
            .or(parent_workspace)
            .map_or(path, |workspace| &workspace.path);
        let owner = &directories[dependency_root];
        let manager = resolve_manager(directory, owner, &mut report.problems);
        let mut evidence = owner.evidence.clone();
        if path != dependency_root {
            evidence.extend(directory.evidence.clone());
        }
        if own_workspace.is_some() && parent_workspace.is_some() {
            report.problems.push(format!(
                "Nested workspace {} overlaps its parent installation root; select a non-overlapping workspace layout before importing.",
                safe(path.display())
            ));
        }
        let mut suggestions: Vec<_> = package
            .scripts
            .iter()
            .filter(|(name, command)| startup_candidate(name) && !command.trim().is_empty())
            .map(|(name, _)| name.clone())
            .collect();
        suggestions.sort_by_key(|name| {
            (
                if name == "dev" {
                    0
                } else if name.starts_with("dev:") {
                    1
                } else {
                    2
                },
                name.clone(),
            )
        });
        if directory.package.is_some() && owner.package.is_none() {
            report.problems.push(format!(
                "Installation root {} has no package.json.",
                safe(dependency_root.display())
            ));
        }
        report.candidates.push(Candidate {
            path: path.clone(),
            dependency_root: dependency_root.clone(),
            name: package.name.clone(),
            scripts: package.scripts.clone(),
            suggestions,
            manager,
            evidence,
        });
    }
    let mut roots_with_commands = BTreeSet::new();
    for candidate in &report.candidates {
        if candidate.path == candidate.dependency_root && !candidate.suggestions.is_empty() {
            roots_with_commands.insert(candidate.path.clone());
        }
    }
    for root in roots_with_commands {
        if report.candidates.iter().any(|candidate| {
            candidate.dependency_root == root
                && candidate.path != root
                && !candidate.suggestions.is_empty()
        }) {
            report.warnings.push(format!(
                "Root scripts in {} may also start member projects. Select the root OR independent members to avoid duplicate starts; script behavior has not been executed or inferred.",
                safe(root.display())
            ));
        }
    }
    if report.candidates.is_empty() && report.problems.is_empty() {
        report.problems.push("No supported package.json project found. Select a Node project or its declared workspace root.".into());
    }
    report.problems.sort();
    report.problems.dedup();
    Ok(report)
}

fn add_workspace(directory: &Directory, workspaces: &mut Vec<Workspace>, report: &mut Report) {
    report.problems.extend(directory.problems.clone());
    report.warnings.extend(directory.warnings.clone());
    if let Some(patterns) = &directory.workspace {
        match Workspace::new(&directory.path, patterns) {
            Ok(workspace) => workspaces.push(workspace),
            Err(error) => report.problems.push(error),
        }
    }
}

fn visit(
    path: &Path,
    depth: usize,
    reader: &mut Reader,
    directories: &mut BTreeMap<PathBuf, Directory>,
    workspaces: &mut Vec<Workspace>,
    report: &mut Report,
) -> Result<(), String> {
    reader.check()?;
    let enclosing = workspaces
        .iter()
        .filter(|workspace| path.starts_with(&workspace.path))
        .max_by_key(|workspace| workspace.path.components().count());
    // An explicitly supplied package remains visible even if excluded from a parent workspace.
    let include = path == report.root || enclosing.is_none_or(|workspace| workspace.contains(path));
    if include {
        match reader.directory(path, &report.declaration_root) {
            Ok(directory) => {
                add_workspace(&directory, workspaces, report);
                directories.insert(path.into(), directory);
            }
            Err(error) => report.problems.push(error),
        }
    }
    let active = workspaces
        .iter()
        .filter(|workspace| path.starts_with(&workspace.path))
        .max_by_key(|workspace| workspace.path.components().count());
    if active.is_some_and(|workspace| !workspace.reachable(path, true)) {
        return Ok(());
    }
    let entries = reader.children(path)?;
    for child in entries {
        let active = workspaces
            .iter()
            .filter(|workspace| child.starts_with(&workspace.path))
            .max_by_key(|workspace| workspace.path.components().count());
        if active.is_some_and(|workspace| !workspace.reachable(&child, false)) {
            continue;
        }
        if depth >= reader.limits.depth {
            return Err(format!(
                "Discovery incomplete: depth limit reached at {}. Select a narrower directory.",
                safe(child.display())
            ));
        }
        visit(&child, depth + 1, reader, directories, workspaces, report)?;
    }
    Ok(())
}

fn resolve_manager(
    directory: &Directory,
    owner: &Directory,
    problems: &mut Vec<String>,
) -> Option<Manager> {
    let mut declarations = Vec::new();
    for entry in if directory.path == owner.path {
        vec![owner]
    } else {
        vec![owner, directory]
    } {
        declarations.extend(entry.managers.clone());
        for issue in &entry.problems {
            problems.push(issue.clone());
        }
    }
    // Prefer explicit versions even when inherited evidence was only a lockfile.
    declarations.sort_by_key(|manager| manager.version.is_none());
    let selected = declarations.first()?.clone();
    for declaration in &declarations {
        if declaration.name != selected.name
            || (declaration.version.is_some()
                && selected.version.is_some()
                && declaration.version != selected.version)
        {
            problems.push(format!(
                "Conflicting package managers for {}: {} ({}) versus {} ({}). Keep declarations and lockfiles consistent; no files were changed.",
                safe(directory.path.display()), safe(&selected.name), safe(selected.source.display()), safe(&declaration.name), safe(declaration.source.display())
            ));
        }
    }
    Some(selected)
}

fn startup_candidate(name: &str) -> bool {
    const ACTIONS: &[&str] = &[
        "stop",
        "restart",
        "shutdown",
        "kill",
        "clean",
        "cleanup",
        "build",
        "test",
        "lint",
        "format",
        "migrate",
        "migration",
        "seed",
        "release",
        "deploy",
        "reset",
        "install",
        "check",
        "typecheck",
        "remove",
        "delete",
        "destroy",
        "prune",
        "teardown",
    ];
    (name == "dev" || name.starts_with("dev:") || name == "start")
        && !name
            .split([':', '-', '_'])
            .any(|part| ACTIONS.contains(&part.to_ascii_lowercase().as_str()))
}

/// Escape controls in untrusted paths, names, scripts and parser diagnostics.
pub fn safe(value: impl std::fmt::Display) -> String {
    value
        .to_string()
        .chars()
        .flat_map(char::escape_debug)
        .collect()
}

pub(crate) fn print_report(report: &Report) {
    println!("Read-only project discovery (no commands executed, no files changed)");
    println!("Directory: {}", safe(report.root.display()));
    println!(
        "Parent inspection boundary: {}",
        safe(report.boundary.display())
    );
    println!(
        "Declaration boundary: {}",
        safe(report.declaration_root.display())
    );
    for candidate in &report.candidates {
        let relative = candidate
            .path
            .strip_prefix(&report.root)
            .expect("Candidates stay inside root.");
        let label = if relative.as_os_str().is_empty() {
            ".".into()
        } else {
            safe(relative.display())
        };
        println!("\nProject: {label}");
        if let Some(name) = &candidate.name {
            println!("  Name: {}", safe(name));
        }
        println!("  Working directory: {}", safe(candidate.path.display()));
        println!(
            "  Dependency root: {}",
            safe(candidate.dependency_root.display())
        );
        if let Some(manager) = &candidate.manager {
            println!(
                "  Package manager: {}{}",
                safe(&manager.name),
                manager
                    .version
                    .as_ref()
                    .map_or(String::new(), |version| format!("@{}", safe(version)))
            );
            println!(
                "  Manager source: {} ({})",
                safe(manager.source.display()),
                safe(&manager.field)
            );
        } else {
            println!("  Package manager: unknown; select a manager after reviewing declarations.");
        }
        for evidence in &candidate.evidence {
            println!(
                "  Evidence: {} ({}) = {}",
                safe(evidence.path.display()),
                safe(&evidence.field),
                safe(&evidence.value)
            );
        }
        if candidate.scripts.is_empty() {
            println!("  No declared scripts; not a runnable service candidate.");
        }
        for (name, command) in &candidate.scripts {
            let suggested = candidate.suggestions.contains(name);
            println!(
                "  Script{}: {} = {}",
                if suggested {
                    " (startup candidate)"
                } else {
                    " (explicit selection only)"
                },
                safe(name),
                safe(command)
            );
            println!(
                "    Source: {} (scripts.{})",
                safe(candidate.path.join("package.json").display()),
                safe(name)
            );
            if let Some(manager) = &candidate.manager {
                println!("    Command: {:?}", [&manager.name, "run", name]);
            }
        }
        if candidate.suggestions.len() > 1 {
            println!(
                "  Selection required: multiple startup candidates; no script selected automatically."
            );
        }
    }
    for warning in &report.warnings {
        println!("\nWarning: {warning}");
    }
    for problem in &report.problems {
        println!("\nProblem: {}", safe(problem));
    }
    println!(
        "\nScan: {}. {} project(s).",
        if report.problems.is_empty() {
            "complete"
        } else {
            "incomplete or conflicting"
        },
        report.candidates.len()
    );
}
