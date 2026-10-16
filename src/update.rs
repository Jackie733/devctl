mod progress;

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use semver::Version;
use serde::Deserialize;
use tokio::{io::AsyncReadExt, process::Command, time::timeout};

use progress::Progress;

const INSTALLER: &str = include_str!("../scripts/devctl-update");
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const TARGET: &str = "x86_64-unknown-linux-musl";
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const TARGET: &str = "aarch64-apple-darwin";
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const TARGET: &str = "x86_64-apple-darwin";
#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(
        target_os = "macos",
        any(target_arch = "aarch64", target_arch = "x86_64")
    )
)))]
const TARGET: &str = "unsupported";
const REPOSITORY: &str = "Jackie733/devctl";
const MAX_BINARY_SIZE: u64 = 128 * 1024 * 1024;

#[derive(Deserialize)]
struct UpdateManifest {
    schema_version: u32,
    version: String,
    assets: Vec<UpdateAsset>,
}

#[derive(Deserialize)]
struct UpdateAsset {
    target: String,
    name: String,
    size: u64,
    sha256: String,
}

impl UpdateManifest {
    fn validate(&self, requested: Option<&Version>) -> Result<(Version, &UpdateAsset), String> {
        if self.schema_version != 1 {
            return Err("Unsupported update manifest schema version.".into());
        }
        let version = stable_version(&self.version)?;
        if self.version != version.to_string() {
            return Err("Update manifest version must use the MAJOR.MINOR.PATCH format.".into());
        }
        if requested.is_some_and(|expected| expected != &version) {
            return Err("Release version does not match the requested version.".into());
        }
        let matches: Vec<_> = self
            .assets
            .iter()
            .filter(|asset| asset.target == TARGET)
            .collect();
        if matches.len() != 1 {
            return Err(format!(
                "Update manifest must contain exactly one '{TARGET}' asset."
            ));
        }
        let asset = matches[0];
        if asset.name != format!("devctl-v{version}-{TARGET}") {
            return Err("Update asset name does not match the release version and target.".into());
        }
        if asset.size == 0 || asset.size > MAX_BINARY_SIZE {
            return Err("Update asset has an invalid size or exceeds its size limit.".into());
        }
        if asset.sha256.len() != 64 || !asset.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("Update asset has an invalid SHA-256 digest.".into());
        }
        Ok((version, asset))
    }
}

fn stable_version(value: &str) -> Result<Version, String> {
    let version = Version::parse(value.strip_prefix('v').unwrap_or(value))
        .map_err(|_| "Expected a stable version such as 1.2.0 or v1.2.0.".to_owned())?;
    if !version.pre.is_empty() || !version.build.is_empty() {
        return Err("Prerelease and build-metadata versions are not supported for updates.".into());
    }
    Ok(version)
}

/// Owns only this update's private temporary directory, never the installation.
struct Staging(PathBuf);

fn install_dir() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(|home| home.join(".local/bin"))
        .ok_or_else(|| "HOME must be an absolute path for updates.".to_owned())
}

impl Staging {
    fn new() -> Result<Self, String> {
        use std::os::unix::fs::DirBuilderExt;

        let bin = install_dir()?;
        fs::create_dir_all(&bin)
            .map_err(|error| format!("Cannot create install directory: {error}"))?;
        for attempt in 0..100 {
            let path = bin.join(format!(".devctl-download-{}-{attempt}", std::process::id()));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!("Cannot create update staging directory: {error}"));
                }
            }
        }
        Err("Cannot allocate an update staging directory.".into())
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Stream subprocess output into a bounded file, including with older curl versions.
async fn download(
    mut command: Command,
    path: &Path,
    limit: u64,
    progress: &mut Progress,
) -> Result<(), String> {
    let mut file =
        fs::File::create(path).map_err(|error| format!("Cannot create download file: {error}"))?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("Cannot start release download: {error}"))?;
    let result = timeout(Duration::from_secs(190), async {
        let mut stdout = child.stdout.take().ok_or("Missing download output.")?;
        let mut stderr = child.stderr.take().ok_or("Missing download diagnostics.")?;
        let mut buffer = [0u8; 64 * 1024];
        let mut diagnostics = [0u8; 8192];
        let (mut out_open, mut err_open) = (true, true);
        let mut total = 0u64;
        let mut refresh = progress::interval();
        while out_open || err_open {
            tokio::select! {
                result = stdout.read(&mut buffer), if out_open => {
                    let count = result.map_err(|error| error.to_string())?;
                    if count == 0 {
                        out_open = false;
                        continue;
                    }
                    total += count as u64;
                    if total > limit {
                        return Err("Release download exceeded its size limit.".to_owned());
                    }
                    file.write_all(&buffer[..count]).map_err(|error| error.to_string())?;
                    progress.received(total);
                }
                result = stderr.read(&mut diagnostics), if err_open => {
                    let count = result.map_err(|error| error.to_string())?;
                    if count == 0 {
                        err_open = false;
                    } else {
                        progress.relay(&diagnostics[..count], true);
                    }
                }
                _ = refresh.tick() => progress.tick(),
            }
        }
        let status = loop {
            tokio::select! {
                status = child.wait() => break status.map_err(|error| error.to_string())?,
                _ = refresh.tick() => progress.tick(),
            }
        };
        if !status.success() {
            return Err(
                "Release download failed; check network access and the release source.".into(),
            );
        }
        Ok(())
    })
    .await;
    match result {
        Ok(Ok(())) => Ok(()),
        error => {
            let _ = child.kill().await;
            match error {
                Ok(Err(error)) => Err(error),
                _ => Err("Release download timed out.".into()),
            }
        }
    }
}

async fn install(
    args: &[&std::ffi::OsStr],
    expected: Option<&Version>,
    progress: &mut Progress,
) -> Result<(), String> {
    let mut command = Command::new("bash");
    command.args(["-c", INSTALLER, "devctl-update"]).args(args);
    command
        .env_remove("DEVCTL_UPDATE_EXPECTED_VERSION")
        .env_remove("BASH_ENV")
        .env_remove("ENV");
    if let Some(version) = expected {
        command.env("DEVCTL_UPDATE_EXPECTED_VERSION", version.to_string());
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("Cannot run the embedded installer (bash required): {error}"))?;
    let status = progress.wait(&mut child).await?;
    if status.success() {
        Ok(())
    } else {
        Err("Update installer failed; inspect the diagnostic above.".into())
    }
}

pub async fn run(check: bool, version: Option<&str>, rollback: bool) -> Result<(), String> {
    let mut progress = Progress::new();
    let result = run_with_progress(check, version, rollback, &mut progress).await;
    if result.is_err() {
        progress.fail();
    }
    result
}

async fn run_with_progress(
    check: bool,
    version: Option<&str>,
    rollback: bool,
    progress: &mut Progress,
) -> Result<(), String> {
    if TARGET == "unsupported" {
        return Err("Self-update supports Linux / WSL2 x86_64 and macOS arm64/x86_64 only.".into());
    }
    if rollback {
        install_dir()?;
        progress.begin("Rolling back", None);
        install(&[std::ffi::OsStr::new("--rollback")], None, progress).await?;
        progress.complete("Rollback complete");
        return Ok(());
    }
    let requested = version.map(stable_version).transpose()?;
    fetch_and_update(check, requested.as_ref(), progress).await
}

fn https_get(url: &str) -> Command {
    let mut command = Command::new("curl");
    command.args([
        "--disable",
        "--fail-with-body",
        "--location",
        "--silent",
        "--show-error",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--connect-timeout",
        "10",
        "--max-time",
        "180",
        "--max-redirs",
        "5",
        "--user-agent",
        concat!("devctl/", env!("CARGO_PKG_VERSION")),
        "--",
        url,
    ]);
    command
}

async fn fetch_and_update(
    check: bool,
    requested: Option<&Version>,
    progress: &mut Progress,
) -> Result<(), String> {
    let repo = REPOSITORY;
    let staging = Staging::new()?;
    let endpoint = requested.map_or_else(
        || "latest/download".into(),
        |version| format!("download/v{version}"),
    );
    let metadata = staging.0.join("update.json");
    progress.begin("Checking release", None);
    download(
        https_get(&format!(
            "https://github.com/{repo}/releases/{endpoint}/update.json"
        )),
        &metadata,
        64 * 1024,
        progress,
    )
    .await
    .map_err(|error| format!(
        "{error} Cannot fetch update.json from the public release. Older releases without this manifest require the external installer; the installed program and backup are unchanged."
    ))?;
    let manifest: UpdateManifest =
        serde_json::from_slice(&fs::read(metadata).map_err(|error| error.to_string())?)
            .map_err(|error| format!("Invalid update manifest: {error}"))?;
    let (version, asset) = manifest.validate(requested)?;
    progress.complete("Release checked");
    let current = stable_version(env!("CARGO_PKG_VERSION"))?;
    println!("Current: {current}\nRelease: {version}\nSource: {repo}");
    if check {
        println!(
            "{}",
            if version > current {
                "An update is available."
            } else if version == current {
                "This version is current."
            } else {
                "The selected release is older than this program."
            }
        );
        return Ok(());
    }
    if requested.is_none() && version <= current {
        println!("No newer stable release; installation unchanged.");
        return Ok(());
    }
    let base = format!("https://github.com/{repo}/releases/download/v{version}");
    let binary = staging.0.join(&asset.name);
    progress.begin("Downloading", Some(asset.size));
    download(
        https_get(&format!("{base}/{}", asset.name)),
        &binary,
        asset.size,
        progress,
    )
    .await?;
    if fs::metadata(&binary)
        .map_err(|error| error.to_string())?
        .len()
        != asset.size
    {
        return Err("Downloaded program size does not match update.json.".into());
    }
    progress.complete(&format!(
        "Downloaded {:.1} MiB",
        asset.size as f64 / (1024.0 * 1024.0)
    ));
    let digest = asset.sha256.to_ascii_lowercase();
    println!(
        "Installing into ~/.local/bin/devctl. Existing backgrounds keep their old code; explicitly shut down workspaces before switching versions."
    );
    progress.begin("Verifying and installing", None);
    install(
        &[binary.as_os_str(), std::ffi::OsStr::new(&digest)],
        Some(&version),
        progress,
    )
    .await?;
    progress.complete(&format!("Update complete · {current} -> {version}"));
    let backup = install_dir()?.join("devctl.previous");
    if backup.is_file() {
        let _ = writeln!(std::io::stdout(), "Backup: {}", backup.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_versions_reject_unsafe_paths_and_unsupported_channels() {
        assert_eq!(stable_version("v1.12.3").unwrap(), Version::new(1, 12, 3));
        for value in ["../1.2.3", "1.2", "1.2.3-rc.1", "1.2.3+local", "01.2.3", ""] {
            assert!(stable_version(value).is_err(), "{value}");
        }
        assert!(Version::new(1, 10, 0) > Version::new(1, 9, 0));
    }

    #[test]
    fn manifest_selects_the_supported_asset_and_checks_requested_version() {
        let mut manifest: UpdateManifest = serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "version": "1.2.0",
            "assets": [{
                "target": TARGET,
                "name": format!("devctl-v1.2.0-{TARGET}"),
                "size": 100,
                "sha256": "AB".repeat(32)
            }]
        }))
        .unwrap();
        let other = serde_json::json!({
            "target": "another-platform", "name": "ignored", "size": 100, "sha256": "AB".repeat(32)
        });
        // A multi-platform release must still select only this client's target.
        manifest
            .assets
            .insert(0, serde_json::from_value(other).unwrap());
        let (version, asset) = manifest.validate(None).unwrap();
        assert_eq!(version, Version::new(1, 2, 0));
        assert_eq!(asset.size, 100);
        assert!(manifest.validate(Some(&version)).is_ok());
        assert!(manifest.validate(Some(&Version::new(1, 3, 0))).is_err());
    }
}
