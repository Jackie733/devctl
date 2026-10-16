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

const INSTALLER: &str = include_str!("../scripts/devctl-update");
const TARGET: &str = "x86_64-unknown-linux-musl";
const REPOSITORY: &str = "Jackie733/devctl";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    state: String,
    size: u64,
}

impl Release {
    fn validate(&self, requested: Option<&Version>) -> Result<Version, String> {
        let version = stable_version(&self.tag_name)?;
        if self.draft || self.prerelease {
            return Err("Only published stable releases can be installed.".into());
        }
        if self.tag_name != format!("v{version}") {
            return Err("Release tag must use the vMAJOR.MINOR.PATCH format.".into());
        }
        if requested.is_some_and(|expected| expected != &version) {
            return Err("Release version does not match the requested version.".into());
        }
        self.asset(&format!("devctl-v{version}-{TARGET}"), 128 * 1024 * 1024)?;
        self.asset("SHA256SUMS", 64 * 1024)?;
        Ok(version)
    }

    fn asset(&self, name: &str, max_size: u64) -> Result<(), String> {
        let matches: Vec<_> = self
            .assets
            .iter()
            .filter(|asset| asset.name == name)
            .collect();
        if matches.len() != 1 {
            return Err(format!("Release must contain exactly one '{name}' asset."));
        }
        let asset = matches[0];
        if asset.state != "uploaded" || asset.size == 0 || asset.size > max_size {
            return Err(format!(
                "Release asset '{name}' is unavailable or exceeds its size limit."
            ));
        }
        Ok(())
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

fn checksum(manifest: &str, name: &str) -> Result<String, String> {
    let mut found = None;
    for line in manifest.lines() {
        let Some((hash, file)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let file = file
            .trim_start()
            .strip_prefix('*')
            .unwrap_or(file.trim_start());
        if file != name {
            continue;
        }
        if found.is_some() || hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(format!("Invalid or duplicate SHA-256 entry for '{name}'."));
        }
        found = Some(hash.to_ascii_lowercase());
    }
    found.ok_or_else(|| format!("SHA256SUMS has no checksum for '{name}'."))
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
async fn download(mut command: Command, path: &Path, limit: u64) -> Result<(), String> {
    let mut file =
        fs::File::create(path).map_err(|error| format!("Cannot create download file: {error}"))?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("Cannot start release download: {error}"))?;
    let result = timeout(Duration::from_secs(190), async {
        let mut stdout = child.stdout.take().ok_or("Missing download output.")?;
        let mut buffer = [0u8; 64 * 1024];
        let mut total = 0u64;
        loop {
            let count = stdout
                .read(&mut buffer)
                .await
                .map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            total += count as u64;
            if total > limit {
                return Err("Release download exceeded its size limit.".to_owned());
            }
            file.write_all(&buffer[..count])
                .map_err(|error| error.to_string())?;
        }
        let status = child.wait().await.map_err(|error| error.to_string())?;
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

async fn install(args: &[&std::ffi::OsStr], expected: Option<&Version>) -> Result<(), String> {
    let mut command = Command::new("bash");
    command.args(["-c", INSTALLER, "devctl-update"]).args(args);
    command
        .env_remove("DEVCTL_UPDATE_EXPECTED_VERSION")
        .env_remove("BASH_ENV")
        .env_remove("ENV");
    if let Some(version) = expected {
        command.env("DEVCTL_UPDATE_EXPECTED_VERSION", version.to_string());
    }
    let status =
        command.kill_on_drop(true).status().await.map_err(|error| {
            format!("Cannot run the embedded installer (bash required): {error}")
        })?;
    if status.success() {
        Ok(())
    } else {
        Err("Update installer failed; inspect the diagnostic above.".into())
    }
}

pub async fn run(check: bool, version: Option<&str>, rollback: bool) -> Result<(), String> {
    if std::env::consts::OS != "linux" || std::env::consts::ARCH != "x86_64" {
        return Err("Self-update currently supports Linux / WSL2 x86_64 only.".into());
    }
    if rollback {
        install_dir()?;
        return install(&[std::ffi::OsStr::new("--rollback")], None).await;
    }
    let requested = version.map(stable_version).transpose()?;
    fetch_and_update(check, requested.as_ref()).await
}

fn https_get(url: &str) -> Command {
    let mut command = Command::new("curl");
    command.args([
        "--disable",
        "--fail",
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

async fn fetch_and_update(check: bool, requested: Option<&Version>) -> Result<(), String> {
    let repo = REPOSITORY;
    let staging = Staging::new()?;
    let endpoint = requested.map_or_else(|| "latest".into(), |version| format!("tags/v{version}"));
    let metadata = staging.0.join("release.json");
    download(
        https_get(&format!(
            "https://api.github.com/repos/{repo}/releases/{endpoint}"
        )),
        &metadata,
        1024 * 1024,
    )
    .await
    .map_err(|error| format!("{error} Release metadata requires a reachable public repository with a published release; private repositories are not supported by this updater."))?;
    let release: Release =
        serde_json::from_slice(&fs::read(metadata).map_err(|error| error.to_string())?)
            .map_err(|error| format!("Invalid release metadata: {error}"))?;
    let version = release.validate(requested)?;
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
    let name = format!("devctl-v{version}-{TARGET}");
    let base = format!("https://github.com/{repo}/releases/download/v{version}");
    let manifest_path = staging.0.join("SHA256SUMS");
    download(
        https_get(&format!("{base}/SHA256SUMS")),
        &manifest_path,
        64 * 1024,
    )
    .await?;
    let manifest = fs::read_to_string(manifest_path)
        .map_err(|error| format!("Cannot read SHA256SUMS: {error}"))?;
    let digest = checksum(&manifest, &name)?;
    let binary = staging.0.join(&name);
    download(
        https_get(&format!("{base}/{name}")),
        &binary,
        128 * 1024 * 1024,
    )
    .await?;
    println!(
        "Installing into ~/.local/bin/devctl. Existing backgrounds keep their old code; explicitly shut down workspaces before switching versions."
    );
    install(
        &[binary.as_os_str(), std::ffi::OsStr::new(&digest)],
        Some(&version),
    )
    .await
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
    fn checksums_require_one_exact_asset_and_a_valid_digest() {
        let hash = "AB".repeat(32);
        let manifest = format!("{hash}  other\n{hash} *devctl\n");
        assert_eq!(checksum(&manifest, "devctl").unwrap(), hash.to_lowercase());
        assert!(checksum(&manifest, "missing").is_err());
        assert!(checksum(&format!("{manifest}{hash}  devctl\n"), "devctl").is_err());
        assert!(checksum("not-a-hash  devctl\n", "devctl").is_err());
        assert!(checksum(&format!("{hash}  devctl.extra"), "devctl").is_err());
    }

    #[test]
    fn releases_must_match_requested_stable_version_and_have_complete_assets() {
        let mut release = Release {
            tag_name: "v1.2.0".into(),
            draft: false,
            prerelease: false,
            assets: vec![
                Asset {
                    name: format!("devctl-v1.2.0-{TARGET}"),
                    state: "uploaded".into(),
                    size: 100,
                },
                Asset {
                    name: "SHA256SUMS".into(),
                    state: "uploaded".into(),
                    size: 100,
                },
            ],
        };
        assert_eq!(release.validate(None).unwrap(), Version::new(1, 2, 0));
        assert!(release.validate(Some(&Version::new(1, 3, 0))).is_err());
        release.draft = true;
        assert!(release.validate(None).is_err());
        release.draft = false;
        release.assets[0].size = 129 * 1024 * 1024;
        assert!(release.validate(None).is_err());
        release.assets.remove(0);
        assert!(release.validate(None).is_err());
    }
}
