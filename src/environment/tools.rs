use std::{
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{Check, NodeEnvironment, State, probe};
use crate::discovery::safe;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stamp {
    dev: u64,
    inode: u64,
    len: u64,
    modified: i64,
    nanos: i64,
}

fn stamp(path: &Path) -> Result<Stamp, String> {
    let metadata = fs::metadata(path).map_err(|error| {
        format!(
            "Cannot inspect selected tool {}: {error}",
            safe(path.display())
        )
    })?;
    Ok(Stamp {
        dev: metadata.dev(),
        inode: metadata.ino(),
        len: metadata.len(),
        modified: metadata.mtime(),
        nanos: metadata.mtime_nsec(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Tool {
    pub name: String,
    pub version: String,
    pub cli: PathBuf,
    root: PathBuf,
    package_digest: String,
    cli_digest: String,
    engines: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Binding {
    pub schema: u32,
    pub environment: NodeEnvironment,
    pub os: String,
    pub arch: String,
    pub node: PathBuf,
    pub node_version: String,
    node_stamp: Stamp,
    pub manager: Tool,
    pub npm: Option<Tool>,
}

pub(super) fn read_text(path: &Path, cap: usize) -> Result<String, String> {
    let resolved = path
        .canonicalize()
        .map_err(|error| format!("Cannot resolve {}: {error}", safe(path.display())))?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(&resolved)
        .map_err(|error| format!("Cannot read {}: {error}", safe(path.display())))?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > cap as u64 {
        return Err(format!(
            "Expected a bounded regular tool/declaration file: {}",
            safe(path.display())
        ));
    }
    let mut bytes = Vec::new();
    file.take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > cap {
        return Err("Tool/declaration file exceeds its byte limit.".into());
    }
    String::from_utf8(bytes).map_err(|_| "Tool/declaration file is not UTF-8.".into())
}

pub(super) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn node_candidate(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("Select an absolute Node executable path.".into());
    }
    if path.components().any(|part| part.as_os_str() == "shims")
        || path.to_string_lossy().contains("/.volta/bin/")
    {
        return Err(
            "Version-manager shims are not executed; select the actual installed Node binary."
                .into(),
        );
    }
    let resolved = path
        .canonicalize()
        .map_err(|error| format!("Node executable is missing: {error}"))?;
    if resolved.file_name().is_some_and(|name| name != "node") {
        return Err(
            "Unrecognized Node shim/name; select an actual native binary named node.".into(),
        );
    }
    let metadata = fs::metadata(&resolved).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err("Selected Node is not an executable regular file.".into());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(&resolved)
        .map_err(|error| error.to_string())?;
    let mut header = [0; 32];
    file.read_exact(&mut header)
        .map_err(|_| "Selected Node is not a recognized native executable.".to_owned())?;
    let native = if cfg!(target_os = "linux") {
        &header[..4] == b"\x7fELF"
            && header[5] == 1
            && u16::from_le_bytes([header[18], header[19]])
                == if cfg!(target_arch = "x86_64") {
                    62
                } else {
                    183
                }
    } else {
        header[..4] == [0xcf, 0xfa, 0xed, 0xfe]
            && u32::from_le_bytes(header[4..8].try_into().expect("four bytes"))
                == if cfg!(target_arch = "aarch64") {
                    0x0100000c
                } else {
                    0x01000007
                }
    };
    if !native {
        return Err("Selected Node is a script/shim or a different native architecture; it was not executed.".into());
    }
    Ok(resolved)
}

pub(super) async fn node_version(path: &Path) -> Result<String, String> {
    let node = node_candidate(path)?;
    let text = probe::run(&node, &["--version".into()]).await?;
    let raw = text.strip_prefix('v').unwrap_or(&text);
    let version = semver::Version::parse(raw)
        .map_err(|_| "Selected Node returned an invalid version.".to_owned())?;
    if !version.pre.is_empty() || !version.build.is_empty() {
        return Err("Select a stable Node version.".into());
    }
    Ok(raw.into())
}

pub(super) fn local_manager(node: &Path, name: &str) -> Result<String, String> {
    find_tool(name, None, &node_candidate(node)?)
        .map(|tool| tool.version)
        .map_err(|check| check.detail)
}

/// A missing installation can be prepared later; a known incompatible tuple
/// must not become an import choice merely because the selected Node is absent.
pub(super) fn import_compatibility(
    environment: &NodeEnvironment,
    explicit: Option<&Path>,
) -> Result<(), String> {
    let node = if let Some(node) = explicit {
        Some(node_candidate(node)?)
    } else {
        std::env::var_os("PATH")
            .into_iter()
            .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
            .filter(|path| path.is_absolute())
            .take(64)
            .find_map(|path| node_candidate(&path.join("node")).ok())
    };
    let Some(node) = node else {
        return Ok(());
    };
    let Ok(tool) = find_tool(
        &environment.package_manager,
        Some(&environment.package_manager_version),
        &node,
    ) else {
        return Ok(());
    };
    if tool.version != environment.package_manager_version {
        return Ok(());
    }
    let version: node_semver::Version = environment
        .version
        .parse()
        .map_err(|_| "Invalid selected Node version.".to_owned())?;
    let range: node_semver::Range = tool
        .engines
        .parse()
        .map_err(|_| "Unknown manager Node requirement.".to_owned())?;
    if !version.satisfies(&range) {
        return Err(format!(
            "Selected {} {} requires Node {}, incompatible with selected {} (source {}).",
            tool.name,
            tool.version,
            safe(&tool.engines),
            environment.version,
            safe(tool.root.join("package.json").display())
        ));
    }
    Ok(())
}

fn tool(path: &Path, name: &str) -> Result<Tool, String> {
    let cli = path.canonicalize().map_err(|error| error.to_string())?;
    for root in cli
        .parent()
        .ok_or("Manager CLI has no parent.")?
        .ancestors()
        .take(6)
    {
        let package_path = root.join("package.json");
        if !package_path.exists() {
            continue;
        }
        let text = read_text(&package_path, 1024 * 1024)?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|_| "Invalid installed package-manager metadata.".to_owned())?;
        if value.get("name").and_then(Value::as_str) != Some(name) {
            return Err(
                "Unrecognized package-manager shim; only actual npm/pnpm packages are probed."
                    .into(),
            );
        }
        let entry = match value.get("bin") {
            Some(Value::String(entry)) => Some(entry.as_str()),
            Some(map) => map.get(name).and_then(Value::as_str),
            None => None,
        }
        .ok_or("Package-manager metadata has no matching CLI entry.")?;
        let entry = root
            .join(entry)
            .canonicalize()
            .map_err(|error| error.to_string())?;
        if entry != cli || !cli.starts_with(root) {
            return Err("Package-manager CLI escapes or differs from its declared entry.".into());
        }
        let version = value
            .get("version")
            .and_then(Value::as_str)
            .ok_or("Package-manager version missing.")?
            .to_owned();
        semver::Version::parse(&version)
            .map_err(|_| "Invalid installed package-manager version.".to_owned())?;
        let engines = value
            .get("engines")
            .and_then(|value| value.get("node"))
            .and_then(Value::as_str)
            .ok_or("Package manager has no verifiable Node requirement.")?
            .to_owned();
        engines
            .parse::<node_semver::Range>()
            .map_err(|_| "Unsupported package-manager Node requirement.".to_owned())?;
        let entry_text = read_text(&cli, 1024 * 1024)?;
        return Ok(Tool {
            name: name.into(),
            version,
            cli: cli.clone(),
            root: root.into(),
            package_digest: digest(text.as_bytes()),
            cli_digest: digest(entry_text.as_bytes()),
            engines,
        });
    }
    Err("No recognized installed package-manager package found; shims are not executed.".into())
}

fn candidates(name: &str, version: Option<&str>, node: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    // Probe only an exact cached package, never activate Corepack or enumerate its cache.
    if name == "pnpm"
        && let Some(version) = version
    {
        let cache = std::env::var_os("COREPACK_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("XDG_CACHE_HOME")
                    .map(|path| PathBuf::from(path).join("node/corepack"))
            })
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|path| PathBuf::from(path).join(".cache/node/corepack"))
            });
        if let Some(cache) = cache.filter(|path| path.is_absolute()) {
            paths.push(cache.join("v1/pnpm").join(version).join("bin/pnpm.cjs"));
        }
    }
    if let Some(parent) = node.parent() {
        paths.push(parent.join(name));
        if let Some(root) = parent.parent() {
            paths.push(root.join(format!(
                "lib/node_modules/{name}/bin/{}",
                if name == "npm" {
                    "npm-cli.js"
                } else {
                    "pnpm.cjs"
                }
            )));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(
            std::env::split_paths(&path)
                .filter(|path| path.is_absolute())
                .map(|path| path.join(name)),
        );
    }
    paths
}

fn find_tool(name: &str, version: Option<&str>, node: &Path) -> Result<Tool, Check> {
    let mut mismatched = None;
    let mut unknown = false;
    let paths = candidates(name, version, node);
    let truncated = paths.len() > 64;
    for path in paths.into_iter().take(64) {
        if fs::symlink_metadata(&path).is_err() {
            continue;
        }
        match tool(&path, name) {
            Ok(tool) if version.is_none_or(|version| tool.version == version) => return Ok(tool),
            Ok(tool) => mismatched = Some(tool),
            Err(_) => unknown = true,
        }
    }
    if truncated {
        return Err(Check::new(
            "package manager",
            State::Unknown,
            "Package-manager candidate limit reached; select a Node installation containing the required actual manager.",
        ));
    }
    if let Some(tool) = mismatched {
        return Ok(tool);
    }
    Err(Check::new(
        "package manager",
        if unknown {
            State::Unknown
        } else {
            State::Missing
        },
        format!(
            "No recognized {name} {} CLI found; Corepack and other shims were not executed. Select/install the actual declared package manager separately.",
            version.unwrap_or("(version unspecified)")
        ),
    ))
}

pub(super) async fn discover(
    environment: &NodeEnvironment,
    explicit: Option<&Path>,
) -> Result<Binding, Check> {
    tokio::time::timeout(
        std::time::Duration::from_secs(8),
        discover_inner(environment, explicit),
    )
    .await
    .map_err(|_| {
        Check::new(
            "tool discovery",
            State::Unknown,
            "Local tool discovery exceeded its 8-second deadline.",
        )
    })?
}

async fn discover_inner(
    environment: &NodeEnvironment,
    explicit: Option<&Path>,
) -> Result<Binding, Check> {
    if explicit.is_none() {
        match super::storage::load_optional(environment) {
            Ok(Some(binding)) => {
                binding
                    .verify(environment)
                    .await
                    .map_err(|error| Check::new("local binding", State::Unknown, error))?;
                return Ok(binding);
            }
            Err(error) => return Err(Check::new("local binding", State::Unknown, error)),
            Ok(None) => {}
        }
    }
    let paths = explicit.map_or_else(
        || {
            std::env::var_os("PATH")
                .map(|path| {
                    std::env::split_paths(&path)
                        .filter(|path| path.is_absolute())
                        .map(|path| path.join("node"))
                        .collect()
                })
                .unwrap_or_default()
        },
        |path| vec![path.into()],
    );
    let mut found = None;
    let truncated = paths.len() > 64;
    let mut unknown = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for path in paths.into_iter().take(64) {
        if fs::symlink_metadata(&path).is_err() {
            continue;
        }
        match node_candidate(&path) {
            Ok(node) if seen.insert(node.clone()) => {
                match probe::run(&node, &["--version".into()]).await {
                    Ok(text) => {
                        let version = text.strip_prefix('v').unwrap_or(&text);
                        if semver::Version::parse(version).is_err() {
                            unknown.push("Node returned an invalid version.".into());
                            continue;
                        }
                        found = Some((node, version.to_owned()));
                        if version == environment.version {
                            break;
                        }
                    }
                    Err(error) => unknown.push(error),
                }
            }
            Ok(_) => {}
            Err(error) => unknown.push(error),
        }
    }
    if truncated
        && found
            .as_ref()
            .is_none_or(|(_, version)| version != &environment.version)
    {
        return Err(Check::new(
            "Node",
            State::Unknown,
            "Node candidate limit reached; use --node to select an absolute installed binary.",
        ));
    }
    let Some((node, node_version)) = found else {
        return Err(Check::new("Node", if unknown.is_empty() { State::Missing } else { State::Unknown }, unknown.into_iter().next().unwrap_or_else(|| "No native Node executable found. Use doctor/setup --node /absolute/path/to/node.".into())));
    };
    if node_version != environment.version {
        return Err(Check::new(
            "Node",
            State::Incompatible,
            format!(
                "required {}; observed {} at {}",
                environment.version,
                node_version,
                safe(node.display())
            ),
        ));
    }
    let manager = find_tool(
        &environment.package_manager,
        Some(&environment.package_manager_version),
        &node,
    )?;
    let npm = if environment.package_manager == "npm" {
        Some(manager.clone())
    } else {
        find_tool("npm", None, &node).ok()
    };
    let binding = Binding {
        schema: 1,
        environment: environment.clone(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        node_stamp: stamp(&node).map_err(|error| Check::new("Node", State::Unknown, error))?,
        node,
        node_version,
        manager,
        npm,
    };
    for check in binding.checks(environment) {
        check
            .require()
            .map_err(|error| Check::new("tool compatibility", State::Incompatible, error))?;
    }
    // Probe the actual selected CLI, never a PATH/Corepack wrapper.
    binding
        .probe_tools()
        .await
        .map_err(|error| Check::new("tool probe", State::Unknown, error))?;
    Ok(binding)
}

impl Binding {
    pub fn checks(&self, environment: &NodeEnvironment) -> Vec<Check> {
        let node_ok = self.node_version == environment.version;
        let manager_ok = self.manager.name == environment.package_manager
            && self.manager.version == environment.package_manager_version;
        let mut checks = vec![
            Check::new(
                "Node",
                if node_ok {
                    State::Satisfied
                } else {
                    State::Incompatible
                },
                format!(
                    "required {}; observed {} at {}",
                    environment.version,
                    self.node_version,
                    safe(self.node.display())
                ),
            ),
            Check::new(
                "package manager",
                if manager_ok {
                    State::Satisfied
                } else {
                    State::Incompatible
                },
                format!(
                    "required {} {}; observed {} {} at {}",
                    environment.package_manager,
                    environment.package_manager_version,
                    self.manager.name,
                    self.manager.version,
                    safe(self.manager.cli.display())
                ),
            ),
        ];
        for tool in std::iter::once(&self.manager)
            .chain(self.npm.iter())
            .take(if self.manager.name == "npm" { 1 } else { 2 })
        {
            let version: node_semver::Version = self
                .node_version
                .parse()
                .expect("probed exact Node version");
            let range: node_semver::Range =
                tool.engines.parse().expect("validated tool requirement");
            checks.push(Check::new(
                "tool Node compatibility",
                if version.satisfies(&range) {
                    State::Satisfied
                } else {
                    State::Incompatible
                },
                format!(
                    "{} {} requires Node {}",
                    tool.name, tool.version, tool.engines
                ),
            ));
        }
        checks
    }

    pub fn metadata(&self) -> Result<(), String> {
        if node_candidate(&self.node)? != self.node || stamp(&self.node)? != self.node_stamp {
            return Err("Bound Node path/file changed; explicitly review a new binding.".into());
        }
        for expected in std::iter::once(&self.manager).chain(self.npm.iter()) {
            let actual = tool(&expected.cli, &expected.name)?;
            if actual.root != expected.root
                || actual.version != expected.version
                || actual.package_digest != expected.package_digest
                || actual.cli_digest != expected.cli_digest
                || actual.engines != expected.engines
            {
                return Err("Bound package-manager metadata/entry changed; explicitly review a new binding.".into());
            }
        }
        Ok(())
    }

    async fn probe_tools(&self) -> Result<(), String> {
        for tool in std::iter::once(&self.manager)
            .chain(self.npm.iter())
            .take(if self.manager.name == "npm" { 1 } else { 2 })
        {
            let output = probe::run(
                &self.node,
                &[tool.cli.clone().into_os_string(), "--version".into()],
            )
            .await?;
            if output != tool.version {
                return Err("Package-manager probe disagrees with installed metadata.".into());
            }
        }
        Ok(())
    }

    pub async fn verify(&self, environment: &NodeEnvironment) -> Result<(), String> {
        if self.schema != 1
            || self.environment != *environment
            || self.os != std::env::consts::OS
            || self.arch != std::env::consts::ARCH
        {
            return Err("Local Node binding belongs to a different version/platform.".into());
        }
        if self.node_version != environment.version
            || self.manager.name != environment.package_manager
            || self.manager.version != environment.package_manager_version
            || self.npm.as_ref().is_some_and(|npm| npm.name != "npm")
            || (self.manager.name == "npm"
                && self.npm.as_ref().is_none_or(|npm| {
                    npm.cli != self.manager.cli || npm.version != self.manager.version
                }))
        {
            return Err(
                "Local binding tool identities/versions do not match its declared environment."
                    .into(),
            );
        }
        self.metadata()?;
        super::constraints::external_settings(&self.node)?;
        let output = probe::run(&self.node, &["--version".into()]).await?;
        if output.strip_prefix('v').unwrap_or(&output) != self.node_version {
            return Err("Bound Node reports a changed version; no PATH fallback was used.".into());
        }
        self.probe_tools().await?;
        for check in self.checks(environment) {
            check.require()?;
        }
        Ok(())
    }
}
