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
    let text = succeeds(fixture.run(&[]));
    assert!(text.contains("Configuration diff"));
    assert!(!text.contains("DO_NOT_READ"));
    assert!(!fixture.config.parent().unwrap().exists());
    assert!(fs::read_dir(&fixture.home).unwrap().next().is_none());
    fails(fixture.run(&["--apply"]), "requires --yes");
    assert!(!fixture.config.parent().unwrap().exists());
    let text = succeeds(fixture.apply(&[]));
    assert!(text.contains("Saved configuration"));
    assert!(text.contains("Commands use the existing environment"));
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
    succeeds(fixture.apply(&["--script", ".=dev"]));
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
    let preview = succeeds(fixture.run(&["--replace", "custom project"]));
    assert!(preview.contains("~ services.custom service.cmd"));
    succeeds(fixture.apply(&["--replace", "custom project"]));
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
    succeeds(fixture.apply(&["--replace", "custom project"]));
    assert_eq!(fs::read(&fixture.config).unwrap(), bytes);
}

#[test]
fn workspace_selections_save_only_paths_and_commands_without_importing_libraries() {
    let fixture = Fixture::new();
    fixture.workspace();
    succeeds(fixture.apply(&["--project", "apps/web", "--script", "apps/web=dev"]));
    succeeds(fixture.apply(&["--project", "apps/api"]));
    let value = fixture.config_value();
    assert_eq!(value["projects"].as_table().unwrap().len(), 2);
    assert_eq!(value["services"].as_table().unwrap().len(), 2);
    assert!(!value.as_table().unwrap().contains_key("node_environments"));
    for project in value["projects"].as_table().unwrap().values() {
        assert!(!project.as_table().unwrap().contains_key("dependency_root"));
    }
    let bytes = fs::read(&fixture.config).unwrap();
    succeeds(fixture.apply(&["--project", "apps/api", "--project", "apps/web"]));
    assert_eq!(fs::read(&fixture.config).unwrap(), bytes);
}

#[test]
fn ambiguous_commands_require_selection_but_environment_constraints_do_not_block_import() {
    let fixture = Fixture::new();
    fixture.workspace();
    fails(fixture.apply(&[]), "Selection required");
    fails(fixture.apply(&["--project", "apps/web"]), "--script");
    fails(
        fixture.apply(&["--project", "packages/library"]),
        "No startup script was invented",
    );
    fails(
        fixture.apply(&["--project", "apps/api", "--script", "apps/web=dev"]),
        "unselected",
    );
    fails(
        fixture.apply(&["--project", "../repository with spaces/apps/api"]),
        "relative paths",
    );
    fails(
        fixture.apply(&["--project", "apps/api", "--replace", "absent"]),
        "Unknown replacement",
    );
    fixture.write(
        "packages/library/package.json",
        &json!({"engines":{"node":"<24"}}).to_string(),
    );
    succeeds(fixture.apply(&["--project", "apps/api"]));
}

#[test]
fn import_needs_no_runtime_version_or_installed_tools_and_only_selects_a_manager_name() {
    let fixture = Fixture::new();
    succeeds(fixture.apply(&[]));
    let fresh = Fixture::new();
    fresh.write(
        "package.json",
        &json!({"scripts":{"dev":"node server"},"engines":{"node":"unrecognized"}}).to_string(),
    );
    fails(fresh.apply(&[]), "Package manager command is unknown");
    succeeds(fresh.apply(&["--package-manager", "npm"]));
    assert_eq!(
        fresh.config_value()["services"]["repository-with-spaces"]["cmd"][0].as_str(),
        Some("npm")
    );
}

#[test]
fn canonical_aliases_reuse_ids_and_configuration_aliases_remain_links() {
    let fixture = Fixture::new();
    succeeds(fixture.apply(&[]));
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
    succeeds(fixture.apply(&["--project", "foo.bar", "--project", "foo_bar"]));
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
    succeeds(fixture.run(&["--replace", "custom project"]));
    fails(
        fixture.apply(&["--replace", "custom project"]),
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
    succeeds(fixture.apply(&["--replace", "custom project"]));
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
    let output = fixture.apply(&[]);
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
        fixture.apply(&["--replace", "custom project"]),
        "hard-linked",
    );
    fs::remove_file(link).unwrap();
    let original = fs::read(&fixture.config).unwrap();
    let canonical = fixture.config.canonicalize().unwrap();
    let mut sidecar = canonical.as_os_str().to_os_string();
    sidecar.push(".devctl.edit.lock");
    fs::create_dir(PathBuf::from(sidecar)).unwrap();
    fails(fixture.apply(&["--replace", "custom project"]), "edit lock");
    assert_eq!(fs::read(&fixture.config).unwrap(), original);
}

#[test]
fn removed_manifests_are_retained_and_independent_nested_projects_can_be_imported() {
    let fixture = Fixture::new();
    fixture.workspace();
    succeeds(fixture.apply(&["--project", "apps/web", "--script", "apps/web=dev"]));
    fs::remove_file(fixture.repository.join("apps/web/package.json")).unwrap();
    let text = succeeds(fixture.apply(&["--project", "apps/api"]));
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
    succeeds(other.apply(&["--project", ".", "--project", "nested"]));
    assert_eq!(
        other.config_value()["projects"].as_table().unwrap().len(),
        2
    );
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
        "manager",
        "add_apply",
        "add_cancel",
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
        if mode == "manager" {
            fixture.write(
                "package.json",
                &json!({"scripts":{"dev":"node server"}}).to_string(),
            );
        }
        if mode.starts_with("add_") {
            // Explicit add must not inspect even an invalid manifest.
            fixture.write("package.json", "INVALID_UNUSED_MANIFEST");
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
            .args(["import", fixture.repository.to_str().unwrap()])
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
    succeeds(fixture.run(&[]));
}

#[test]
fn inline_config_tables_keep_unrelated_text_and_invalid_manager_flags_are_rejected() {
    let fixture = Fixture::new();
    let original = "version=1\nworkspace = { name = 'inline workspace' } # KEEP INLINE\nlogs = { enabled = false }\nprojects = { custom = { path = 'repository with spaces' } }\nservices = { server = { project = 'custom', cmd = ['sleep','60'], autostart = false } }\n";
    fs::write(&fixture.config, original).unwrap();
    for flags in [
        &["--node-version", "^24"][..],
        &["--package-manager", "npm@12.0.0"][..],
    ] {
        let output = fixture.apply(flags);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert_eq!(fs::read_to_string(&fixture.config).unwrap(), original);
    }
    succeeds(fixture.apply(&["--replace", "custom"]));
    let text = fs::read_to_string(&fixture.config).unwrap();
    assert!(text.contains("workspace = { name = 'inline workspace' } # KEEP INLINE"));
    assert!(text.contains("projects = { custom = { path = 'repository with spaces' } }"));
    assert_eq!(
        fixture.config_value()["services"]["server"]["autostart"].as_bool(),
        Some(false)
    );
    succeeds(fixture.command(&["validate"]).output().unwrap());
}

#[test]
fn explicit_add_works_without_manifests_and_preserves_exact_arguments_and_existing_edits() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.repository.join("package.json")).unwrap();
    let add = |extra: &[&str]| {
        let mut command = fixture.command(&[
            "add",
            fixture.repository.to_str().unwrap(),
            "--name",
            "custom runner",
        ]);
        command.args(extra).args([
            "--",
            "uninstalled-runtime",
            "argument with spaces",
            "--apply",
            "literal $HOME; printf unsafe",
        ]);
        command.output().unwrap()
    };
    succeeds(add(&[]));
    assert!(!fixture.config.exists());
    fails(add(&["--apply"]), "requires --yes");
    succeeds(add(&["--apply", "--yes"]));
    let value = fixture.config_value();
    assert_eq!(
        value["services"]["custom runner"]["cmd"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "uninstalled-runtime",
            "argument with spaces",
            "--apply",
            "literal $HOME; printf unsafe"
        ]
    );
    let mut bytes = fs::read_to_string(&fixture.config).unwrap();
    bytes.push_str("# User comment\n");
    fs::write(&fixture.config, &bytes).unwrap();
    let inode = fs::metadata(&fixture.config).unwrap().ino();
    succeeds(add(&["--apply", "--yes"]));
    assert_eq!(fs::read_to_string(&fixture.config).unwrap(), bytes);
    assert_eq!(fs::metadata(&fixture.config).unwrap().ino(), inode);
    assert!(fs::read_dir(&fixture.home).unwrap().next().is_none());
}

#[test]
fn explicit_add_validates_directory_command_and_name_without_overwriting_entries() {
    let fixture = Fixture::new();
    fixture.legacy();
    let original = fs::read(&fixture.config).unwrap();
    let other = fixture.root.join("another project");
    fs::create_dir(&other).unwrap();
    let output = fixture
        .command(&[
            "add",
            other.to_str().unwrap(),
            "--name",
            "custom project",
            "--apply",
            "--yes",
            "--",
            "sleep",
            "60",
        ])
        .output()
        .unwrap();
    fails(output, "name already exists");
    fails(
        fixture
            .command(&["add", "/dev/null", "--", "sleep", "60"])
            .output()
            .unwrap(),
        "must be a directory",
    );
    fails(
        fixture
            .command(&["add", other.to_str().unwrap(), "--", ""])
            .output()
            .unwrap(),
        "non-empty command",
    );
    assert_eq!(fs::read(&fixture.config).unwrap(), original);
    succeeds(
        fixture
            .command(&[
                "add",
                other.to_str().unwrap(),
                "--name",
                "other",
                "--apply",
                "--yes",
                "--",
                "sleep",
                "60",
            ])
            .output()
            .unwrap(),
    );
    assert!(
        fs::read_to_string(&fixture.config)
            .unwrap()
            .contains("# KEEP HEADER")
    );
    assert_eq!(
        fixture.config_value()["projects"].as_table().unwrap().len(),
        2
    );
}

#[test]
fn retired_environment_interfaces_fail_explicitly_instead_of_silently_changing_execution() {
    let fixture = Fixture::new();
    let original = fixture.legacy();
    for args in [
        vec!["doctor"],
        vec!["setup"],
        vec!["start", "--all", "--allow-unverified-dependencies"],
        vec!["run", "--all", "--allow-unverified-dependencies"],
    ] {
        let output = fixture.command(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{output:?}");
    }
    for content in [
        format!("{original}\nnode_environment='old'\n"),
        format!("{original}\n[node_environments.old]\nversion='24.0.0'\n"),
        original.replace(
            "path = 'repository with spaces'",
            "dependency_root='.'\npath = 'repository with spaces'",
        ),
    ] {
        fs::write(&fixture.config, &content).unwrap();
        fails(
            fixture.command(&["run", "--all"]).output().unwrap(),
            "Managed environment configuration was removed",
        );
        assert_eq!(fs::read_to_string(&fixture.config).unwrap(), content);
    }
    fs::write(&fixture.config, &original).unwrap();
    let help = succeeds(fixture.command(&["--help"]).output().unwrap());
    assert!(!help.contains("doctor") && !help.contains("setup"));
    assert!(help.contains("add"));
}

#[test]
fn imported_services_run_inherited_commands_without_environment_checks_and_reconnect() {
    use std::time::{Duration, Instant};
    let fixture = Fixture::new();
    let bin = fixture.root.join("bin");
    fs::create_dir(&bin).unwrap();
    let marker = fixture.root.join("ARGUMENTS");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$@\" > '{}'\nprintf 'service output\\n'\nexec /bin/sleep 60\n",
        marker.display()
    );
    fs::write(bin.join("npm"), script).unwrap();
    fs::set_permissions(bin.join("npm"), fs::Permissions::from_mode(0o700)).unwrap();
    // A failing node shim and impossible constraints must never gate service startup.
    let probe_marker = fixture.root.join("PROBE_EXECUTED");
    fs::write(
        bin.join("node"),
        format!("#!/bin/sh\ntouch '{}'\nexit 99\n", probe_marker.display()),
    )
    .unwrap();
    fs::set_permissions(bin.join("node"), fs::Permissions::from_mode(0o700)).unwrap();
    fixture.write("package.json", &json!({"packageManager":"npm@999.0.0", "engines":{"node":"impossible"}, "scripts":{"dev":"user startup"}}).to_string());
    succeeds(fixture.apply(&[]));
    let run = |args: &[&str]| {
        fixture
            .command(args)
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .output()
            .unwrap()
    };
    succeeds(run(&["start", "--all"]));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !marker.exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let first = succeeds(run(&["status"]));
    assert!(first.contains("Running"));
    succeeds(run(&["start", "--all"]));
    assert_eq!(succeeds(run(&["status"])), first);
    assert_eq!(
        fs::read_to_string(marker).unwrap(),
        format!(
            "{}\nrun\ndev\n",
            fixture.repository.canonicalize().unwrap().display()
        )
    );
    assert!(!probe_marker.exists());
    assert!(!fixture.repository.join("node_modules").exists());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if succeeds(run(&["logs", "repository-with-spaces"])).contains("service output") {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    succeeds(run(&["stop", "--all"]));
    assert!(succeeds(run(&["status"])).contains("Stopped"));
    succeeds(run(&["shutdown"]));
}
