//! Local Node checks and explicit bindings; this module never downloads tools.

mod constraints;
mod probe;
mod storage;
mod tools;

use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    config::{Config, Service},
    discovery::safe,
};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NodeEnvironment {
    pub version: String,
    pub package_manager: String,
    pub package_manager_version: String,
}

impl NodeEnvironment {
    pub fn validate(&self) -> Result<(), String> {
        for text in [&self.version, &self.package_manager_version] {
            let version = semver::Version::parse(text).map_err(|_| {
                format!(
                    "Node environments require exact stable versions, got {}",
                    safe(text)
                )
            })?;
            if !version.pre.is_empty() || !version.build.is_empty() {
                return Err(
                    "Node environments require exact stable versions without metadata.".into(),
                );
            }
        }
        let major = semver::Version::parse(&self.package_manager_version)
            .expect("validated version")
            .major;
        if !matches!(
            (self.package_manager.as_str(), major),
            ("npm", 10 | 11) | ("pnpm", 10)
        ) {
            return Err("Managed Node environments support npm 10/11 or pnpm 10.".into());
        }
        Ok(())
    }

    fn key(&self) -> String {
        format!(
            "node-{}-{}-{}-{}-{}",
            self.version,
            self.package_manager,
            self.package_manager_version,
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    }
}

#[derive(Debug, Clone)]
pub struct Managed {
    pub environment: NodeEnvironment,
    pub project: PathBuf,
    pub dependency_root: PathBuf,
}

#[derive(Debug)]
pub struct Launch {
    node: PathBuf,
    cli: PathBuf,
    path: OsString,
    shell: PathBuf,
    _root_lock: Arc<File>,
    _binding_lock: Arc<File>,
}

impl Launch {
    pub fn command(&self, script: &str) -> tokio::process::Command {
        let mut command = tokio::process::Command::new(&self.node);
        for (key, _) in std::env::vars_os() {
            if matches!(
                key.to_string_lossy()
                    .to_ascii_lowercase()
                    .replace('-', "_")
                    .as_str(),
                "npm_config_manage_package_manager_versions" | "npm_config_script_shell"
            ) {
                command.env_remove(key);
            }
        }
        command.args([
            self.cli.as_os_str(),
            std::ffi::OsStr::new("run"),
            std::ffi::OsStr::new(script),
        ]);
        command
            .env("PATH", &self.path)
            .env("npm_config_manage_package_manager_versions", "false")
            .env("npm_config_script_shell", &self.shell)
            .env("COREPACK_ENABLE_NETWORK", "0");
        command
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Satisfied,
    Missing,
    Incompatible,
    Unknown,
}

impl State {
    fn label(self) -> &'static str {
        match self {
            Self::Satisfied => "satisfied",
            Self::Missing => "missing",
            Self::Incompatible => "incompatible",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone)]
struct Check {
    item: String,
    state: State,
    detail: String,
}

impl Check {
    fn new(item: &str, state: State, detail: impl Into<String>) -> Self {
        Self {
            item: item.into(),
            state,
            detail: detail.into(),
        }
    }
    fn require(&self) -> Result<(), String> {
        if self.state == State::Satisfied {
            Ok(())
        } else {
            Err(format!(
                "{} {}: {}",
                self.item,
                self.state.label(),
                self.detail
            ))
        }
    }
}

fn selected(config: &Config, ids: &[String]) -> Result<BTreeMap<String, Service>, String> {
    if ids.len().max(if ids.is_empty() {
        config.services.len()
    } else {
        0
    }) > 200
    {
        return Err("Select at most 200 services for environment checks.".into());
    }
    let ids: Vec<_> = if ids.is_empty() {
        config.services.keys().cloned().collect()
    } else {
        ids.to_vec()
    };
    ids.into_iter()
        .map(|id| {
            config
                .services
                .get(&id)
                .cloned()
                .map(|service| (id.clone(), service))
                .ok_or_else(|| format!("Unknown service '{}'.", safe(id)))
        })
        .collect()
}

pub async fn interruptible(
    future: impl std::future::Future<Output = Result<(), String>>,
) -> Result<(), String> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut interrupt = signal(SignalKind::interrupt()).map_err(|error| error.to_string())?;
    let mut terminate = signal(SignalKind::terminate()).map_err(|error| error.to_string())?;
    tokio::select! {
        result = future => result,
        _ = interrupt.recv() => Err("Environment operation cancelled; active probes were terminated. No installation was performed.".into()),
        _ = terminate.recv() => Err("Environment operation cancelled; active probes were terminated. No installation was performed.".into()),
    }
}

pub async fn doctor(config: &Config, ids: &[String], node: Option<&Path>) -> Result<(), String> {
    let services = selected(config, ids)?;
    let checked = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut failed = false;
    for (id, service) in services {
        println!("Service: {} | checked_utc_s={checked}", safe(&id));
        let Some(managed) = &service.managed else {
            println!(
                "  environment unknown: legacy command has no node_environment; inherited execution is unchanged."
            );
            failed = true;
            continue;
        };
        let facts = constraints::inspect(managed, &service.cmd[2]);
        let checks = match facts {
            Ok(facts) => {
                let mut checks = facts.checks.clone();
                match tools::discover(&managed.environment, node).await {
                    Ok(binding) => {
                        checks.extend(binding.checks(&managed.environment));
                        checks.push(match constraints::external_settings(&binding.node) {
                            Ok(()) => Check::new("external tool settings", State::Satisfied, "No unsupported runtime-switching/hook settings were found in supported configuration locations."),
                            Err(error) => Check::new("external tool settings", State::Unknown, error),
                        });
                        checks.push(storage::binding_check(&managed.environment, &binding));
                    }
                    Err(check) => checks.push(check),
                }
                checks
            }
            Err(error) => vec![Check::new("repository declarations", State::Unknown, error)],
        };
        for check in checks {
            println!(
                "  {} {}: {}",
                safe(&check.item),
                check.state.label(),
                safe(&check.detail)
            );
            failed |= check.state != State::Satisfied;
        }
        println!(
            "  Next: setup can explicitly bind compatible local tools; dependency/tool installation is not implemented yet."
        );
    }
    if failed {
        Err("Environment checks contain missing, incompatible or unknown items. No tools were installed and no bindings were changed.".into())
    } else {
        Ok(())
    }
}

/// Only adoption of existing tools is available in this slice, never installation.
pub async fn setup(
    config: &Config,
    ids: &[String],
    node: Option<&Path>,
    apply: bool,
    yes: bool,
) -> Result<(), String> {
    let services = selected(config, ids)?;
    let mut bindings = BTreeMap::new();
    let mut reviewed = BTreeMap::new();
    for (id, service) in &services {
        let managed = service
            .managed
            .as_ref()
            .ok_or_else(|| format!("Service '{}' has no managed Node environment.", safe(id)))?;
        let facts = constraints::inspect(managed, &service.cmd[2])?;
        reviewed.insert(id.clone(), facts.fingerprint.clone());
        for check in &facts.checks {
            if check.item != "dependencies" {
                check.require()?;
            }
        }
        let binding = tools::discover(&managed.environment, node)
            .await
            .map_err(|check| format!("{}: {}", check.state.label(), check.detail))?;
        for check in binding.checks(&managed.environment) {
            check.require()?;
        }
        constraints::external_settings(&binding.node)?;
        println!(
            "Bind existing tools for {}: Node {} at {}, {} {} at {}",
            safe(id),
            binding.node_version,
            safe(binding.node.display()),
            binding.manager.name,
            binding.manager.version,
            safe(binding.manager.cli.display())
        );
        let recorded = storage::binding_check(&managed.environment, &binding);
        println!(
            "  Current local binding {}: {}",
            recorded.state.label(),
            safe(&recorded.detail)
        );
        bindings.insert(
            managed.environment.key(),
            (managed.environment.clone(), binding),
        );
    }
    println!(
        "This action only records local tool bindings and creates private launchers. It does not install tools/dependencies or edit project files."
    );
    if !apply {
        return Ok(());
    }
    if !yes && !confirm_binding().await? {
        return Err("Binding cancelled; no bindings changed.".into());
    }
    // Recheck reviewed source/tool evidence after the user has made a selection.
    for (id, service) in &services {
        let managed = service
            .managed
            .as_ref()
            .expect("managed selection validated");
        let facts = constraints::inspect(managed, &service.cmd[2])?;
        if reviewed[id] != facts.fingerprint {
            return Err("Repository declarations changed after the preview; review setup again. No bindings changed.".into());
        }
        for check in facts.checks {
            if check.item != "dependencies" {
                check.require()?;
            }
        }
    }
    for (environment, binding) in bindings.values() {
        binding.verify(environment).await?;
    }
    for (_, (environment, binding)) in bindings {
        storage::adopt(&environment, &binding)?;
    }
    println!(
        "Local bindings verified. Run doctor to check dependencies; application readiness is not implied."
    );
    Ok(())
}

async fn confirm_binding() -> Result<bool, String> {
    use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind};
    use futures_util::StreamExt;
    use std::io::{IsTerminal, Write};
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err(
            "Non-interactive binding requires --apply --yes after reviewing the preview.".into(),
        );
    }
    print!("Apply these bindings? [y/N] ");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    struct Raw;
    impl Drop for Raw {
        fn drop(&mut self) {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
    crossterm::terminal::enable_raw_mode().map_err(|e| e.to_string())?;
    let _raw = Raw;
    let mut events = EventStream::new();
    while let Some(event) = events.next().await {
        if let Event::Key(key) = event.map_err(|error| error.to_string())?
            && key.kind == KeyEventKind::Press
        {
            println!("\r");
            return Ok(matches!(key.code, KeyCode::Char('y' | 'Y')));
        }
    }
    Ok(false)
}

/// Preflight the complete new-instance batch before Manager registers any child.
pub async fn prepare(
    services: BTreeMap<String, Service>,
    allow_unverified: bool,
) -> Result<BTreeMap<String, Service>, String> {
    if services.len() > 200 {
        return Err("Select at most 200 services for environment checks.".into());
    }
    tokio::time::timeout(
        Duration::from_secs(8),
        prepare_inner(services, allow_unverified),
    )
    .await
    .map_err(|_| {
        "Environment preflight exceeded its 8-second batch deadline; no new service was started."
            .to_owned()
    })?
}

async fn prepare_inner(
    services: BTreeMap<String, Service>,
    allow_unverified: bool,
) -> Result<BTreeMap<String, Service>, String> {
    let mut prepared = services;
    // Keep dependency evidence stable against cooperating setup operations.
    let mut roots = BTreeMap::new();
    for managed in prepared
        .values()
        .filter_map(|service| service.managed.as_ref())
    {
        if !roots.contains_key(&managed.dependency_root) {
            roots.insert(
                managed.dependency_root.clone(),
                Arc::new(storage::root_lock(&managed.dependency_root)?),
            );
        }
    }
    let facts_input = prepared.clone();
    let facts = tokio::task::spawn_blocking(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        let mut result = BTreeMap::new();
        let mut reports = BTreeMap::new();
        for (id, service) in facts_input {
            if std::time::Instant::now() >= deadline {
                return Err("Repository preflight deadline exceeded.".into());
            }
            if let Some(managed) = &service.managed {
                if !reports.contains_key(&managed.dependency_root) {
                    reports.insert(
                        managed.dependency_root.clone(),
                        crate::discovery::scan(&managed.dependency_root)?,
                    );
                }
                result.insert(
                    id,
                    constraints::inspect_report(
                        managed,
                        &service.cmd[2],
                        &reports[&managed.dependency_root],
                    )?,
                );
            }
        }
        Ok::<_, String>(result)
    })
    .await
    .map_err(|e| format!("Environment checks failed: {e}"))??;
    let mut verified = BTreeMap::new();
    for (id, service) in &mut prepared {
        let Some(managed) = &service.managed else {
            continue;
        };
        for check in &facts[id].checks {
            if check.item == "dependencies" && check.state == State::Unknown && allow_unverified {
                continue;
            }
            check.require().map_err(|error| format!("Service '{}': {error}. Run doctor/setup; unknown dependencies require --allow-unverified-dependencies for this start only.", safe(id)))?;
        }
        let key = managed.environment.key();
        if !verified.contains_key(&key) {
            let lock = Arc::new(storage::binding_lock(&managed.environment)?);
            let binding = storage::load(&managed.environment)?;
            binding.verify(&managed.environment).await?;
            verified.insert(key.clone(), (binding, lock));
        }
        let (binding, binding_lock) = &verified[&key];
        let launchers = storage::directory(&managed.environment)?.join("bin");
        let mut paths = vec![
            launchers,
            binding
                .node
                .parent()
                .ok_or("Node has no parent directory.")?
                .to_path_buf(),
        ];
        if let Some(path) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&path));
        }
        service.launch = Some(Arc::new(Launch {
            node: binding.node.clone(),
            cli: binding.manager.cli.clone(),
            path: std::env::join_paths(paths).map_err(|error| error.to_string())?,
            shell: storage::directory(&managed.environment)?.join("bin/devctl-script-shell"),
            _root_lock: roots[&managed.dependency_root].clone(),
            _binding_lock: binding_lock.clone(),
        }));
    }
    Ok(prepared)
}
