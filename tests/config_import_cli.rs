use serde_json::json;
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    home: PathBuf,
    repository: PathBuf,
    config: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from("/tmp").join(format!(
            "devctl-import-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let home = root.join("home");
        let repository = root.join("repository with spaces");
        fs::create_dir_all(repository.join(".git")).unwrap();
        fs::create_dir(&home).unwrap();
        let config = root.join("workspace.toml");
        let fixture = Self {
            root,
            home,
            repository,
            config,
        };
        fixture.write("package.json", &json!({"packageManager":"npm@11.0.0","engines":{"node":">=24 <25"},"scripts":{"dev":"node server.cjs"}}).to_string());
        fixture
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.repository.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_devctl"));
        command
            .args(["--config", self.config.to_str().unwrap()])
            .args(args)
            .current_dir(&self.root)
            .env("HOME", &self.home)
            .env("PATH", "/usr/bin:/bin")
            .env_remove("COREPACK_HOME")
            .env_remove("XDG_CACHE_HOME")
            .env_remove("NODE_OPTIONS");
        command
    }
    fn import(&self, args: &[&str]) -> Command {
        let mut command = self.command(&["import", self.repository.to_str().unwrap()]);
        command.args(args);
        command
    }
    fn run(&self, args: &[&str]) -> Output {
        self.import(args).output().unwrap()
    }
    fn apply(&self, args: &[&str]) -> Output {
        let mut command = self.import(args);
        command.args(["--apply", "--yes"]).output().unwrap()
    }
    fn config_value(&self) -> toml::Value {
        toml::from_str(&fs::read_to_string(&self.config).unwrap()).unwrap()
    }
    fn legacy(&self) -> String {
        fs::create_dir_all(self.repository.join("custom cwd")).unwrap();
        let text = String::from(
            "# KEEP HEADER\nversion = 1\n[workspace]\nname = 'Hand edited' # KEEP WORKSPACE\n[logs]\nenabled = false\n[projects.'custom project']\npath = 'repository with spaces' # KEEP PATH\n[services.'custom service']\nproject = 'custom project'\ncmd = ['sleep', '60'] # KEEP COMMAND COMMENT\ncwd = 'custom cwd'\nautostart = false # KEEP GROUP\n",
        );
        fs::write(&self.config, &text).unwrap();
        text
    }
    fn workspace(&self) {
        self.write(
            "package.json",
            &json!({"packageManager":"pnpm@10.3.0"}).to_string(),
        );
        self.write(
            "pnpm-workspace.yaml",
            "packages:\n  - 'apps/*'\n  - 'packages/*'\n",
        );
        self.write(
            "apps/web/package.json",
            &json!({"scripts":{"dev":"vite","start":"node server"}}).to_string(),
        );
        self.write(
            "apps/api/package.json",
            &json!({"scripts":{"dev":"node api"}}).to_string(),
        );
        self.write("packages/library/package.json", "{}");
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.command(&["shutdown"]).output();
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn succeeds(output: Output) -> String {
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}
fn fails(output: Output, expected: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains(expected), "{text}");
}

#[test]
fn preview_is_read_only_and_new_configuration_requires_explicit_noninteractive_apply() {
    let mut fixture = Fixture::new();
    fixture.config = fixture.root.join("new parent/workspace.toml");
    fixture.write(".env", "PRIVATE_ENV_TOKEN=DO_NOT_READ");
    let manifest = fs::read(fixture.repository.join("package.json")).unwrap();
    let text = succeeds(fixture.run(&["--node-version", "24.0.0"]));
    assert!(text.contains("Configuration diff"));
    assert!(!text.contains("DO_NOT_READ"));
    assert!(!fixture.config.parent().unwrap().exists());
    assert!(fs::read_dir(&fixture.home).unwrap().next().is_none());
    fails(
        fixture.run(&["--node-version", "24.0.0", "--apply"]),
        "requires --yes",
    );
    assert!(!fixture.config.parent().unwrap().exists());
    let text = succeeds(fixture.apply(&["--node-version", "24.0.0"]));
    assert!(text.contains("Saved configuration"));
    assert!(text.contains("Older devctl versions reject"));
    let config = fixture.config_value();
    assert_eq!(config["projects"].as_table().unwrap().len(), 1);
    assert_eq!(
        config["services"]["repository-with-spaces"]["cmd"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>(),
        ["npm", "run", "dev"]
    );
    assert_eq!(fs::metadata(&fixture.config).unwrap().mode() & 0o777, 0o600);
    assert_eq!(
        fs::read(fixture.repository.join("package.json")).unwrap(),
        manifest
    );
    assert!(!fixture.repository.join("node_modules").exists());
    assert!(fixture.command(&["status"]).output().unwrap().status.code() == Some(1));
    succeeds(fixture.command(&["validate"]).output().unwrap());
}

#[test]
fn repeat_import_preserves_custom_fields_ids_comments_bytes_and_inode() {
    let fixture = Fixture::new();
    let original = fixture.legacy();
    let metadata = fs::metadata(&fixture.config).unwrap();
    succeeds(fixture.apply(&["--script", ".=dev", "--node-version", "24.0.0"]));
    assert_eq!(fs::read_to_string(&fixture.config).unwrap(), original);
    assert_eq!(fs::metadata(&fixture.config).unwrap().ino(), metadata.ino());
    assert_eq!(
        fs::metadata(&fixture.config).unwrap().mtime_nsec(),
        metadata.mtime_nsec()
    );
    assert!(
        !fixture
            .config_value()
            .as_table()
            .unwrap()
            .contains_key("node_environments")
    );
}

#[test]
fn replacement_changes_only_reviewed_fields_and_retains_the_existing_group_and_comments() {
    let fixture = Fixture::new();
    fixture.legacy();
    fs::set_permissions(&fixture.config, fs::Permissions::from_mode(0o640)).unwrap();
    let preview =
        succeeds(fixture.run(&["--replace", "custom project", "--node-version", "24.0.0"]));
    assert!(preview.contains("~ services.custom service.cmd"));
    succeeds(fixture.apply(&["--replace", "custom project", "--node-version", "24.0.0"]));
    let text = fs::read_to_string(&fixture.config).unwrap();
    for marker in [
        "# KEEP HEADER",
        "# KEEP WORKSPACE",
        "# KEEP PATH",
        "# KEEP COMMAND COMMENT",
        "# KEEP GROUP",
    ] {
        assert!(text.contains(marker), "{text}");
    }
    let value = fixture.config_value();
    assert_eq!(value["workspace"]["name"].as_str(), Some("Hand edited"));
    assert_eq!(
        value["services"]["custom service"]["autostart"].as_bool(),
        Some(false)
    );
    assert_eq!(
        value["services"]["custom service"]["cmd"][0].as_str(),
        Some("npm")
    );
    assert_eq!(
        value["projects"]["custom project"]["path"].as_str(),
        Some("repository with spaces")
    );
    assert_eq!(fs::metadata(&fixture.config).unwrap().mode() & 0o777, 0o640);
    let bytes = fs::read(&fixture.config).unwrap();
    succeeds(fixture.apply(&["--replace", "custom project", "--node-version", "24.0.0"]));
    assert_eq!(fs::read(&fixture.config).unwrap(), bytes);
}

#[test]
fn workspace_selections_share_one_environment_and_root_without_importing_libraries() {
    let fixture = Fixture::new();
    fixture.workspace();
    succeeds(fixture.apply(&[
        "--project",
        "apps/web",
        "--script",
        "apps/web=dev",
        "--node-version",
        "24.0.0",
    ]));
    succeeds(fixture.apply(&["--project", "apps/api", "--node-version", "24.0.0"]));
    let value = fixture.config_value();
    assert_eq!(value["projects"].as_table().unwrap().len(), 2);
    assert_eq!(value["services"].as_table().unwrap().len(), 2);
    assert_eq!(value["node_environments"].as_table().unwrap().len(), 1);
    for project in value["projects"].as_table().unwrap().values() {
        assert_eq!(
            project["dependency_root"].as_str(),
            fixture.repository.canonicalize().unwrap().to_str()
        );
    }
    let bytes = fs::read(&fixture.config).unwrap();
    succeeds(fixture.apply(&["--project", "apps/api", "--project", "apps/web"]));
    assert_eq!(fs::read(&fixture.config).unwrap(), bytes);
}

#[test]
fn ambiguity_unknown_flags_and_incompatible_workspace_members_cannot_be_applied() {
    let fixture = Fixture::new();
    fixture.workspace();
    fails(
        fixture.apply(&["--node-version", "24.0.0"]),
        "Selection required",
    );
    fails(
        fixture.apply(&["--project", "apps/web", "--node-version", "24.0.0"]),
        "--script",
    );
    fails(
        fixture.apply(&["--project", "packages/library", "--node-version", "24.0.0"]),
        "No startup script was invented",
    );
    fails(
        fixture.apply(&[
            "--project",
            "apps/api",
            "--script",
            "apps/web=dev",
            "--node-version",
            "24.0.0",
        ]),
        "unselected",
    );
    fails(
        fixture.apply(&[
            "--project",
            "../repository with spaces/apps/api",
            "--node-version",
            "24.0.0",
        ]),
        "relative paths",
    );
    fails(
        fixture.apply(&[
            "--project",
            "apps/api",
            "--replace",
            "absent",
            "--node-version",
            "24.0.0",
        ]),
        "Unknown replacement",
    );
    fixture.write(
        "packages/library/package.json",
        &json!({"engines":{"node":"<24"}}).to_string(),
    );
    fails(
        fixture.apply(&["--project", "apps/api", "--node-version", "24.0.0"]),
        "incompatible",
    );
    assert!(!fixture.config.exists());
}

#[test]
fn missing_versions_stay_unresolved_and_exact_repository_pins_can_resolve_them() {
    let fixture = Fixture::new();
    fails(fixture.run(&[]), "Node selection required");
    fixture.write(".node-version", "24.0.0\n");
    succeeds(fixture.apply(&[]));
    let fresh = Fixture::new();
    fresh.write(
        "package.json",
        &json!({"scripts":{"dev":"node server"}}).to_string(),
    );
    fresh.write("package-lock.json", "{}");
    fails(
        fresh.apply(&["--node-version", "24.0.0"]),
        "version is unresolved",
    );
    succeeds(fresh.apply(&[
        "--node-version",
        "24.0.0",
        "--package-manager",
        "npm@11.0.0",
    ]));
}

#[test]
fn canonical_aliases_reuse_ids_and_configuration_aliases_remain_links() {
    let fixture = Fixture::new();
    succeeds(fixture.apply(&["--node-version", "24.0.0"]));
    let original = fs::read(&fixture.config).unwrap();
    let alias = fixture.root.join("alias");
    symlink(&fixture.repository, &alias).unwrap();
    let config_alias = fixture.root.join("alias.toml");
    symlink(&fixture.config, &config_alias).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_devctl"))
        .args([
            "--config",
            config_alias.to_str().unwrap(),
            "import",
            alias.to_str().unwrap(),
            "--apply",
            "--yes",
        ])
        .current_dir(&fixture.root)
        .env("HOME", &fixture.home)
        .output()
        .unwrap();
    succeeds(out);
    assert_eq!(fs::read(&fixture.config).unwrap(), original);
    assert!(fs::symlink_metadata(config_alias).unwrap().is_symlink());
}

#[test]
fn slug_collisions_get_distinct_stable_path_ids_and_do_not_overwrite_existing_entries() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.repository.join("package.json")).unwrap();
    for name in ["foo.bar", "foo_bar"] {
        fixture.write(
            &format!("{name}/package.json"),
            &json!({"packageManager":"npm@11.0.0","scripts":{"dev":"node server"}}).to_string(),
        );
    }
    succeeds(fixture.apply(&[
        "--project",
        "foo.bar",
        "--project",
        "foo_bar",
        "--node-version",
        "24.0.0",
    ]));
    let value = fixture.config_value();
    let projects = value["projects"].as_table().unwrap();
    assert_eq!(projects.len(), 2);
    assert!(projects.contains_key("foo-bar"));
    assert!(projects.keys().any(|key| key.starts_with("foo-bar-")));
    let bytes = fs::read(&fixture.config).unwrap();
    succeeds(fixture.apply(&["--project", "foo_bar", "--project", "foo.bar"]));
    assert_eq!(fs::read(&fixture.config).unwrap(), bytes);
}

#[test]
fn an_active_or_idle_workspace_blocks_apply_without_stopping_its_service() {
    let fixture = Fixture::new();
    let original = fixture.legacy();
    succeeds(
        fixture
            .command(&["start", "custom service"])
            .output()
            .unwrap(),
    );
    succeeds(fixture.run(&["--replace", "custom project", "--node-version", "24.0.0"]));
    fails(
        fixture.apply(&["--replace", "custom project", "--node-version", "24.0.0"]),
        "workspace is running",
    );
    assert_eq!(fs::read_to_string(&fixture.config).unwrap(), original);
    assert!(succeeds(fixture.command(&["status"]).output().unwrap()).contains("Running"));
    succeeds(
        fixture
            .command(&["stop", "custom service"])
            .output()
            .unwrap(),
    );
    fails(fixture.apply(&[]), "workspace is running");
    succeeds(fixture.command(&["shutdown"]).output().unwrap());
    succeeds(fixture.apply(&["--replace", "custom project", "--node-version", "24.0.0"]));
}

#[test]
fn runners_reread_committed_configuration_after_preflight_even_when_replacement_races() {
    use std::{
        io::Write,
        os::unix::fs::OpenOptionsExt,
        time::{Duration, Instant},
    };
    struct Owner(std::process::Child);
    impl Drop for Owner {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let _ = nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(self.0.id() as i32),
                    nix::sys::signal::Signal::SIGTERM,
                );
                let deadline = Instant::now() + Duration::from_secs(8);
                while self.0.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(10));
                }
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
    for background in [false, true] {
        let fixture = Fixture::new();
        let stale = fixture.root.join("STALE_COMMAND_EXECUTED");
        let fresh = fixture.root.join("FRESH_COMMAND_EXECUTED");
        let content = |marker: &Path| {
            format!(
                "version=1\n[workspace]\nname='reread'\n[logs]\nenabled=false\n[projects.demo]\npath={}\n[services.demo]\nproject='demo'\ncmd={}\n",
                serde_json::to_string(fixture.repository.to_str().unwrap()).unwrap(),
                json!([
                    "sh",
                    "-c",
                    format!("printf executed > '{}'; sleep 60", marker.display())
                ])
            )
        };
        nix::unistd::mkfifo(
            &fixture.config,
            nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
        )
        .unwrap();
        let mut sidecar = fixture.config.as_os_str().to_os_string();
        sidecar.push(".devctl.lock");
        let lock = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(PathBuf::from(sidecar))
            .unwrap();
        lock.try_lock().unwrap();
        let args = if background {
            vec!["supervise", "--detached"]
        } else {
            vec!["run", "demo"]
        };
        let mut owner = Owner(
            fixture
                .command(&args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
        // A FIFO gates only the initial read, without adding production test hooks.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut writer = loop {
            match fs::OpenOptions::new()
                .write(true)
                .custom_flags(nix::libc::O_NONBLOCK)
                .open(&fixture.config)
            {
                Ok(file) => break file,
                Err(error) if error.raw_os_error() == Some(nix::libc::ENXIO) => {
                    assert!(
                        owner.0.try_wait().unwrap().is_none(),
                        "runner exited before validation"
                    );
                    assert!(Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("{error}"),
            }
        };
        writer.write_all(content(&stale).as_bytes()).unwrap();
        let replacement = fixture.root.join("complete.toml");
        fs::write(&replacement, content(&fresh)).unwrap();
        fs::rename(replacement, &fixture.config).unwrap();
        drop(lock);
        drop(writer);
        if background {
            let socket = PathBuf::from(format!("{}.devctl/control.sock", fixture.config.display()));
            while !socket.exists() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(10));
            }
            succeeds(fixture.command(&["start", "demo"]).output().unwrap());
        }
        while !fresh.exists() {
            assert!(
                !stale.exists(),
                "runner executed a configuration read before ownership"
            );
            assert!(
                Instant::now() < deadline,
                "runner did not use committed configuration"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!stale.exists());
        drop(owner);
    }
}

#[test]
fn simultaneous_imports_never_publish_partial_files_or_lose_successful_additions() {
    let fixture = Fixture::new();
    fixture.workspace();
    let mut commands = Vec::new();
    for name in ["apps/api", "apps/web"] {
        commands.push((
            name,
            fixture
                .import(&[
                    "--project",
                    name,
                    "--script",
                    &format!("{name}=dev"),
                    "--node-version",
                    "24.0.0",
                    "--apply",
                    "--yes",
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        ));
    }
    let mut successes = Vec::new();
    for (name, command) in commands {
        let output = command.wait_with_output().unwrap();
        if output.status.success() {
            successes.push(name);
        } else {
            assert_eq!(output.status.code(), Some(1), "{output:?}");
        }
    }
    assert!(!successes.is_empty());
    let config = fixture.config_value();
    for name in successes {
        assert!(
            config["projects"]
                .as_table()
                .unwrap()
                .values()
                .any(|project| Path::new(project["path"].as_str().unwrap())
                    == fixture.repository.join(name).canonicalize().unwrap())
        );
    }
    succeeds(fixture.command(&["validate"]).output().unwrap());
}

#[test]
fn invalid_existing_configs_hardlinks_and_bad_sidecars_do_not_get_replaced() {
    let fixture = Fixture::new();
    fs::write(&fixture.config, "secret_unknown_field = 'PRIVATE_TOKEN'\n").unwrap();
    let output = fixture.apply(&["--node-version", "24.0.0"]);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("PRIVATE_TOKEN"));
    fails(output, "Existing configuration is invalid");
    assert!(
        fs::read_to_string(&fixture.config)
            .unwrap()
            .contains("PRIVATE_TOKEN")
    );
    fixture.legacy();
    let link = fixture.root.join("hardlink.toml");
    fs::hard_link(&fixture.config, &link).unwrap();
    fails(
        fixture.apply(&["--replace", "custom project", "--node-version", "24.0.0"]),
        "hard-linked",
    );
    fs::remove_file(link).unwrap();
    let original = fs::read(&fixture.config).unwrap();
    let canonical = fixture.config.canonicalize().unwrap();
    let mut sidecar = canonical.as_os_str().to_os_string();
    sidecar.push(".devctl.edit.lock");
    fs::create_dir(PathBuf::from(sidecar)).unwrap();
    fails(
        fixture.apply(&["--replace", "custom project", "--node-version", "24.0.0"]),
        "edit lock",
    );
    assert_eq!(fs::read(&fixture.config).unwrap(), original);
}

#[test]
fn removed_manifests_are_reported_and_overlapping_independent_installation_roots_are_refused() {
    let fixture = Fixture::new();
    fixture.workspace();
    succeeds(fixture.apply(&[
        "--project",
        "apps/web",
        "--script",
        "apps/web=dev",
        "--node-version",
        "24.0.0",
    ]));
    fs::remove_file(fixture.repository.join("apps/web/package.json")).unwrap();
    let text = succeeds(fixture.apply(&["--project", "apps/api", "--node-version", "24.0.0"]));
    assert!(text.contains("Retain existing project apps-web"));
    assert_eq!(
        fixture.config_value()["projects"].as_table().unwrap().len(),
        2
    );
    let other = Fixture::new();
    other.write(
        "nested/package.json",
        &json!({"packageManager":"npm@11.0.0","scripts":{"dev":"node nested"}}).to_string(),
    );
    fails(
        other.apply(&[
            "--project",
            ".",
            "--project",
            "nested",
            "--node-version",
            "24.0.0",
        ]),
        "overlap",
    );
    assert!(!other.config.exists());
}

#[test]
fn real_terminal_guides_selection_and_rechecks_sources_and_destination_after_review() {
    for mode in [
        "cancel",
        "apply",
        "source_changed",
        "target_changed",
        "member_changed",
        "signal",
        "choose",
        "choose_cancel",
        "small",
        "versions",
    ] {
        let fixture = Fixture::new();
        if matches!(
            mode,
            "choose" | "choose_cancel" | "small" | "member_changed"
        ) {
            fixture.workspace();
            fixture.write(
                "apps/api/package.json",
                &json!({"scripts":{"dev":"node api","dev:inspect":"node inspect"}}).to_string(),
            );
        }
        if mode == "versions" {
            fixture.write(
                "package.json",
                &json!({"scripts":{"dev":"node server"}}).to_string(),
            );
        }
        let environment = fixture.command(&[]);
        let mut python = Command::new("python3");
        for (key, value) in environment.get_envs() {
            match value {
                Some(value) => {
                    python.env(key, value);
                }
                None => {
                    python.env_remove(key);
                }
            }
        }
        let output = python
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/import_pty.py"))
            .args([
                env!("CARGO_BIN_EXE_devctl"),
                fixture.config.to_str().unwrap(),
                fixture.repository.to_str().unwrap(),
                mode,
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "mode={mode}: {output:?}");
        if matches!(mode, "choose" | "small") {
            let config = fixture.config_value();
            assert_eq!(config["projects"].as_table().unwrap().len(), 1);
            assert_eq!(
                config["services"]["apps-api"]["cmd"][2].as_str(),
                Some("dev:inspect")
            );
        }
    }
}

#[test]
fn default_destination_is_previewed_before_creation_and_invalid_preferred_configs_never_fall_back()
{
    let fixture = Fixture::new();
    let command = |apply: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_devctl"));
        command
            .args([
                "import",
                fixture.repository.to_str().unwrap(),
                "--node-version",
                "24.0.0",
            ])
            .current_dir(&fixture.root)
            .env("HOME", &fixture.home)
            .env("PATH", "/usr/bin:/bin");
        if apply {
            command.args(["--apply", "--yes"]);
        }
        command
    };
    succeeds(command(false).output().unwrap());
    assert!(fs::read_dir(&fixture.home).unwrap().next().is_none());
    succeeds(command(true).output().unwrap());
    let saved = fixture.home.join(".config/devctl/devctl.toml");
    assert!(saved.is_file());
    let bytes = fs::read(&saved).unwrap();
    fs::write(fixture.root.join("devctl.toml"), "INVALID_PREFERRED").unwrap();
    fails(
        command(true).output().unwrap(),
        "Existing configuration is invalid",
    );
    assert_eq!(fs::read(&saved).unwrap(), bytes);
    fs::remove_file(fixture.root.join("devctl.toml")).unwrap();
    fails(
        command(false).env_remove("HOME").output().unwrap(),
        "absolute HOME",
    );
    succeeds(fixture.run(&["--node-version", "24.0.0"]));
}

#[test]
fn inline_config_tables_keep_unrelated_text_and_invalid_version_flags_are_not_ignored_on_reimport()
{
    let fixture = Fixture::new();
    let original = "version=1\nworkspace = { name = 'inline workspace' } # KEEP INLINE\nlogs = { enabled = false }\nprojects = { custom = { path = 'repository with spaces' } }\nservices = { server = { project = 'custom', cmd = ['sleep','60'], autostart = false } }\n";
    fs::write(&fixture.config, original).unwrap();
    for flags in [
        &["--node-version", "^24"][..],
        &["--package-manager", "npm@12.0.0"][..],
    ] {
        let output = fixture.apply(flags);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert_eq!(fs::read_to_string(&fixture.config).unwrap(), original);
    }
    succeeds(fixture.apply(&["--replace", "custom", "--node-version", "24.0.0"]));
    let text = fs::read_to_string(&fixture.config).unwrap();
    assert!(text.contains("workspace = { name = 'inline workspace' } # KEEP INLINE"));
    assert!(text.contains("projects = { custom = { path = 'repository with spaces' } }"));
    assert_eq!(
        fixture.config_value()["services"]["server"]["autostart"].as_bool(),
        Some(false)
    );
    succeeds(fixture.command(&["validate"]).output().unwrap());
}
