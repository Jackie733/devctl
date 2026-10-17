use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt, symlink},
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use super::{
    Check, NodeEnvironment, State,
    tools::{Binding, digest},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn home() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or("An absolute HOME is required for local Node bindings.")?;
    home.canonicalize()
        .map_err(|error| format!("Cannot resolve HOME for local bindings: {error}"))
}

fn base() -> Result<PathBuf, String> {
    Ok(home()?.join(if cfg!(target_os = "macos") {
        "Library/Application Support/devctl/toolchains"
    } else {
        ".local/share/devctl/toolchains"
    }))
}

pub(super) fn directory(environment: &NodeEnvironment) -> Result<PathBuf, String> {
    environment.validate()?;
    Ok(base()?.join("bindings").join(environment.key()))
}

fn chain(path: &Path, create: bool) -> Result<(), String> {
    let home = home()?;
    let relative = path
        .strip_prefix(&home)
        .map_err(|_| "Binding storage is outside HOME.".to_owned())?;
    let mut current = home;
    for part in relative.components() {
        if !matches!(part, std::path::Component::Normal(_)) {
            return Err("Invalid binding storage path.".into());
        }
        current.push(part.as_os_str());
        if create {
            use std::os::unix::fs::DirBuilderExt;
            match fs::DirBuilder::new().mode(0o700).create(&current) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(format!("Cannot create private binding storage: {error}"));
                }
            }
        }
        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| format!("Cannot inspect local binding storage: {error}"))?;
        if !metadata.is_dir()
            || metadata.uid() != nix::unistd::geteuid().as_raw()
            || metadata.permissions().mode() & 0o022 != 0
        {
            return Err("Local binding storage must be owned directories without symlinks or group/world write access.".into());
        }
    }
    Ok(())
}

fn private_file(path: &Path) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| format!("Cannot open local binding: {error}"))?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file()
        || metadata.uid() != nix::unistd::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
        || metadata.len() > 64 * 1024
    {
        return Err("Local binding must be a private owned regular file of at most 64 KiB.".into());
    }
    Ok(file)
}

pub(super) fn load_optional(environment: &NodeEnvironment) -> Result<Option<Binding>, String> {
    let dir = directory(environment)?;
    match fs::symlink_metadata(dir.join("receipt.json")) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Cannot inspect Node binding: {error}")),
        Ok(_) => {}
    }
    chain(&dir, false)?;
    let mut text = String::new();
    private_file(&dir.join("receipt.json"))?
        .take(64 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|_| "Cannot read local binding receipt.".to_owned())?;
    if text.len() > 64 * 1024 {
        return Err("Local binding receipt is oversized.".into());
    }
    let binding: Binding = serde_json::from_str(&text)
        .map_err(|_| "Local binding receipt is malformed.".to_owned())?;
    if binding.environment != *environment
        || binding.schema != 1
        || binding.os != std::env::consts::OS
        || binding.arch != std::env::consts::ARCH
    {
        return Err("Local binding version/platform does not match the configuration.".into());
    }
    validate_launchers(&dir, &binding)?;
    Ok(Some(binding))
}

pub(super) fn load(environment: &NodeEnvironment) -> Result<Binding, String> {
    load_optional(environment)?.ok_or_else(|| "Verified local Node binding is missing. Review setup --node /absolute/path/to/node --apply before starting.".into())
}

pub(super) fn binding_check(environment: &NodeEnvironment, selected: &Binding) -> Check {
    match load_optional(environment) {
        Ok(Some(binding))
            if binding.node == selected.node
                && binding.manager.cli == selected.manager.cli
                && binding.npm.as_ref().map(|tool| &tool.cli)
                    == selected.npm.as_ref().map(|tool| &tool.cli) =>
        {
            if binding.metadata().is_err() {
                return Check::new(
                    "local binding",
                    State::Unknown,
                    "Recorded tool metadata changed. Explicit setup with the same verified paths can refresh the receipt while no managed instance is using it.",
                );
            }
            Check::new(
                "local binding",
                State::Satisfied,
                "Matching local tool binding and launchers are present; tool versions were checked separately.",
            )
        }
        Ok(Some(_)) => Check::new(
            "local binding",
            State::Incompatible,
            "This version tuple is already bound to different paths; do not replace it implicitly.",
        ),
        Ok(None) => Check::new(
            "local binding",
            State::Missing,
            "Tools were found but have not been explicitly bound. Preview setup, then apply the local binding.",
        ),
        Err(error) => Check::new("local binding", State::Unknown, error),
    }
}

fn lock(path: &Path, shared: bool) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| format!("Cannot open environment lock: {error}"))?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file()
        || metadata.uid() != nix::unistd::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
    {
        return Err("Environment lock must be a private owned regular file.".into());
    }
    let result = if shared {
        file.try_lock_shared()
    } else {
        file.try_lock()
    };
    result.map_err(|error| {
        format!("Environment/dependency root is busy or its lock is unavailable: {error}")
    })?;
    Ok(file)
}

pub(super) fn root_lock(root: &Path) -> Result<File, String> {
    let dir = base()?.join("root-locks");
    chain(&dir, true)?;
    lock(
        &dir.join(format!("{}.lock", digest(root.as_os_str().as_bytes()))),
        true,
    )
}

pub(super) fn binding_lock(environment: &NodeEnvironment) -> Result<File, String> {
    let dir = directory(environment)?;
    let parent = dir.parent().expect("binding parent exists");
    chain(parent, false)?;
    lock(&parent.join(format!("{}.lock", environment.key())), true)
}

fn quote(path: &Path) -> Result<String, String> {
    let text = path
        .to_str()
        .ok_or("Tool paths must be UTF-8 to generate Unix launchers.")?;
    Ok(format!("'{}'", text.replace('\'', "'\\''")))
}

fn wrappers(binding: &Binding) -> Result<Vec<(&'static str, String)>, String> {
    let mut files = Vec::new();
    for name in ["npm", "pnpm", "npx"] {
        let cli = match name {
            "npm" => binding.npm.as_ref().map(|tool| tool.cli.clone()),
            "pnpm" if binding.manager.name == "pnpm" => Some(binding.manager.cli.clone()),
            "npx" => binding
                .npm
                .as_ref()
                .and_then(|tool| tool.cli.parent())
                .map(|path| path.join("npx-cli.js"))
                .filter(|path| path.is_file()),
            _ => None,
        };
        let text = if let Some(cli) = cli {
            format!(
                "#!/bin/sh\nexec {} {} \"$@\"\n",
                quote(&binding.node)?,
                quote(&cli)?
            )
        } else {
            format!(
                "#!/bin/sh\nprintf '%s\\n' 'This Node environment has no verified {name}; no PATH fallback is allowed.' >&2\nexit 127\n"
            )
        };
        files.push((name, text));
    }
    files.push(("corepack", "#!/bin/sh\nprintf '%s\\n' 'Corepack activation is disabled in this managed environment.' >&2\nexit 127\n".into()));
    // npm/pnpm prepend project .bin directories before invoking the script shell.
    // Restore tool precedence there, while preserving their remaining shell/PATH semantics.
    files.push((
        "devctl-script-shell",
        format!(
            "#!/bin/sh\nPATH={}:{}:\"$PATH\"\nexport PATH\nexec /bin/sh \"$@\"\n",
            quote(&directory(&binding.environment)?.join("bin"))?,
            quote(
                binding
                    .node
                    .parent()
                    .ok_or("Node has no parent directory.")?
            )?,
        ),
    ));
    Ok(files)
}

fn validate_launchers(dir: &Path, binding: &Binding) -> Result<(), String> {
    chain(&dir.join("bin"), false)?;
    let node = dir.join("bin/node");
    if !fs::symlink_metadata(&node).is_ok_and(|metadata| metadata.file_type().is_symlink())
        || fs::read_link(&node).map_err(|error| error.to_string())? != binding.node
    {
        return Err("Bound Node launcher changed or is missing.".into());
    }
    for (name, expected) in wrappers(binding)? {
        let path = dir.join("bin").join(name);
        let mut text = String::new();
        private_file(&path)?
            .read_to_string(&mut text)
            .map_err(|error| error.to_string())?;
        if text != expected
            || fs::metadata(path)
                .map_err(|error| error.to_string())?
                .mode()
                & 0o111
                == 0
        {
            return Err("Bound package-manager launcher changed or is not executable.".into());
        }
    }
    Ok(())
}

pub(super) fn adopt(environment: &NodeEnvironment, binding: &Binding) -> Result<(), String> {
    let destination = directory(environment)?;
    let parent = destination.parent().expect("binding parent exists");
    chain(parent, true)?;
    let _lock = lock(&parent.join(format!("{}.lock", environment.key())), false)?;
    binding.metadata()?;
    if let Some(existing) = load_optional(environment)? {
        if existing.node == binding.node
            && existing.manager.cli == binding.manager.cli
            && existing.npm.as_ref().map(|tool| &tool.cli)
                == binding.npm.as_ref().map(|tool| &tool.cli)
        {
            if serde_json::to_vec(&existing).map_err(|error| error.to_string())?
                == serde_json::to_vec(binding).map_err(|error| error.to_string())?
            {
                return Ok(());
            }
            let path = destination.join(format!(
                ".receipt-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let result = (|| {
                let bytes = serde_json::to_vec(binding).map_err(|error| error.to_string())?;
                if bytes.len() > 64 * 1024 {
                    return Err("Binding receipt exceeds size limit.".into());
                }
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&path)
                    .map_err(|error| error.to_string())?;
                file.write_all(&bytes)
                    .and_then(|()| file.sync_all())
                    .map_err(|error| error.to_string())?;
                binding.metadata()?;
                fs::rename(&path, destination.join("receipt.json"))
                    .map_err(|error| error.to_string())?;
                File::open(&destination)
                    .and_then(|file| file.sync_all())
                    .map_err(|error| error.to_string())?;
                Ok(())
            })();
            if result.is_err() {
                let _ = fs::remove_file(path);
            }
            return result;
        }
        return Err("The version tuple is already bound to another tool location; implicit replacement is forbidden.".into());
    }
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(
            "Incomplete or conflicting binding directory exists; it was not overwritten.".into(),
        );
    }
    let stage = parent.join(format!(
        ".binding-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        chain(&stage.join("bin"), true)?;
        symlink(&binding.node, stage.join("bin/node")).map_err(|error| error.to_string())?;
        for (name, text) in wrappers(binding)? {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o700)
                .open(stage.join("bin").join(name))
                .map_err(|error| error.to_string())?;
            file.write_all(text.as_bytes())
                .and_then(|()| file.sync_all())
                .map_err(|error| error.to_string())?;
        }
        let bytes = serde_json::to_vec(binding).map_err(|error| error.to_string())?;
        if bytes.len() > 64 * 1024 {
            return Err("Binding receipt exceeds size limit.".into());
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(stage.join("receipt.json"))
            .map_err(|error| error.to_string())?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| error.to_string())?;
        validate_launchers(&stage, binding)?;
        binding.metadata()?;
        File::open(stage.join("bin"))
            .and_then(|file| file.sync_all())
            .map_err(|error| error.to_string())?;
        File::open(&stage)
            .and_then(|file| file.sync_all())
            .map_err(|error| error.to_string())?;
        fs::rename(&stage, &destination)
            .map_err(|error| format!("Cannot publish local binding: {error}"))?;
        File::open(parent)
            .and_then(|file| file.sync_all())
            .map_err(|error| error.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(stage);
    }
    result
}
