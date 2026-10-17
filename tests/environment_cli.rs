use serde_json::json;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{
        Mutex, MutexGuard, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn native_fixture() -> &'static PathBuf {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| {
        let root =
            std::env::temp_dir().join(format!("devctl-node-fixture-build-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let output = root.join("node");
        let target = if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                "aarch64-apple-darwin"
            } else {
                "x86_64-apple-darwin"
            }
        } else {
            "x86_64-unknown-linux-gnu"
        };
        let result = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2024", "--target", target])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/environment_node.rs"))
            .arg("-o")
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        output
    })
}

struct Fixture {
    // Probe deadlines are product behavior. Avoid competing native-code cold starts
    // from unrelated tests; multi-service concurrency is exercised within a fixture.
    _guard: MutexGuard<'static, ()>,
    root: PathBuf,
    home: PathBuf,
    project: PathBuf,
    config: PathBuf,
    node: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static FIXTURE_LOCK: Mutex<()> = Mutex::new(());
        let guard = FIXTURE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        // Keep socket/config paths short on macOS.
        let root = PathBuf::from("/tmp").join(format!(
            "devctl-env-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let home = root.join("home");
        fs::create_dir(&home).unwrap();
        let project = root.join("project");
        fs::create_dir_all(project.join(".git")).unwrap();
        let node = Self::toolchain(&root.join("tools"), "24.0.0", "11.0.0");
        let config = root.join("devctl.toml");
        let fixture = Self {
            _guard: guard,
            root,
            home,
            project,
            config,
            node,
        };
        fixture.manifest(json!({"scripts":{"dev":"fixture"},"engines":{"node":">=24 <25"}}));
        fixture.config("24.0.0", "11.0.0");
        fixture
    }

    fn toolchain(root: &Path, version: &str, npm: &str) -> PathBuf {
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(root.join("version"), version).unwrap();
        let node = root.join("bin/node");
        fs::copy(native_fixture(), &node).unwrap();
        let package = root.join("lib/node_modules/npm");
        fs::create_dir_all(package.join("bin")).unwrap();
        fs::write(package.join("package.json"), json!({"name":"npm","version":npm,"engines":{"node":">=18"},"bin":{"npm":"bin/npm-cli.js","npx":"bin/npx-cli.js"}}).to_string()).unwrap();
        fs::write(package.join("bin/npm-cli.js"), npm).unwrap();
        fs::write(package.join("bin/npx-cli.js"), npm).unwrap();
        symlink(package.join("bin/npm-cli.js"), root.join("bin/npm")).unwrap();
        node
    }

    fn manifest(&self, value: serde_json::Value) {
        fs::write(self.project.join("package.json"), value.to_string()).unwrap();
    }

    fn config(&self, node: &str, npm: &str) {
        fs::write(&self.config, format!("version = 1\n[workspace]\nname = 'environment'\n[logs]\nenabled = false\n[node_environments.local]\nversion = '{node}'\npackage_manager = 'npm'\npackage_manager_version = '{npm}'\n[projects.demo]\npath = '{}'\n[services.demo]\nproject = 'demo'\ncmd = ['npm', 'run', 'dev']\nnode_environment = 'local'\n", self.project.display())).unwrap();
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_devctl"));
        command
            .args(["--config", self.config.to_str().unwrap()])
            .args(args)
            .current_dir(&self.root)
            .env("HOME", &self.home)
            .env_remove("NODE_OPTIONS")
            .env_remove("NODE_PATH")
            .env_remove("COREPACK_HOME")
            .env_remove("XDG_CACHE_HOME")
            .env("PATH", "/usr/bin:/bin");
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    fn adopt(&self) {
        let output = self.run(&[
            "setup",
            "--node",
            self.node.to_str().unwrap(),
            "--apply",
            "--yes",
        ]);
        assert!(output.status.success(), "{output:?}");
    }

    fn observed(&self, project: &Path) -> String {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(value) = fs::read_to_string(project.join("observed")) {
                return value;
            }
            if Instant::now() >= deadline {
                panic!(
                    "service did not start: status={:?} logs={:?}",
                    self.run(&["status"]),
                    self.run(&["logs", "demo"])
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn binding_dir(&self) -> PathBuf {
        self.home.join(if cfg!(target_os = "macos") {
            "Library/Application Support/devctl/toolchains/bindings"
        } else {
            ".local/share/devctl/toolchains/bindings"
        })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.run(&["shutdown"]);
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn doctor_and_preview_are_read_only_then_explicit_binding_enables_managed_startup() {
    let fixture = Fixture::new();
    let original = fs::read(&fixture.config).unwrap();
    let output = fixture.run(&["doctor", "--node", fixture.node.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Node satisfied"), "{text}");
    assert!(text.contains("local binding missing"));
    assert!(fs::read_dir(&fixture.home).unwrap().next().is_none());
    let preview = fixture.run(&["setup", "--node", fixture.node.to_str().unwrap()]);
    assert!(preview.status.success(), "{preview:?}");
    assert!(fs::read_dir(&fixture.home).unwrap().next().is_none());
    fixture.adopt();
    let output = fixture.run(&["doctor"]);
    assert!(output.status.success(), "{output:?}");
    let output = fixture.run(&["start", "demo"]);
    assert!(output.status.success(), "{output:?}");
    let observed = fixture.observed(&fixture.project);
    assert_eq!(observed, "node=v24.0.0\nmanager=11.0.0\nnpm=11.0.0\n");
    assert_eq!(fs::read(&fixture.config).unwrap(), original);
    assert!(fixture.run(&["shutdown"]).status.success());
}

#[test]
fn incompatible_versions_unknown_ranges_and_missing_dependencies_block_before_service_spawn() {
    let fixture = Fixture::new();
    let wrong = fixture.run(&["doctor", "--node", fixture.node.to_str().unwrap()]);
    assert!(
        String::from_utf8(wrong.stdout)
            .unwrap()
            .contains("local binding missing")
    );
    fixture.config("22.0.0", "11.0.0");
    let output = fixture.run(&["doctor", "--node", fixture.node.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("incompatible")
    );
    fixture.config("24.0.0", "11.0.0");
    fixture.adopt();
    fixture.manifest(json!({"scripts":{"dev":"fixture"},"engines":{"node":"lts/*"}}));
    let output = fixture.run(&["start", "demo"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("unknown")
    );
    assert!(!fixture.project.join("observed").exists());
    fixture.manifest(json!({"scripts":{"dev":"fixture"},"dependencies":{"example":"1.0.0"}}));
    let output = fixture.run(&["start", "demo", "--allow-unverified-dependencies"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("missing")
    );
    assert!(!fixture.project.join("observed").exists());
}

#[test]
fn unknown_dependency_inventory_requires_an_explicit_acknowledgement_each_start() {
    let fixture = Fixture::new();
    fixture.adopt();
    fixture.manifest(json!({"scripts":{"dev":"fixture"},"dependencies":{"example":"1.0.0"}}));
    fs::create_dir(fixture.project.join("node_modules")).unwrap();
    assert_eq!(fixture.run(&["start", "demo"]).status.code(), Some(1));
    let output = fixture.run(&["start", "demo", "--allow-unverified-dependencies"]);
    assert!(output.status.success(), "{output:?}");
    fixture.observed(&fixture.project);
    assert!(fixture.run(&["stop", "demo"]).status.success());
    assert_eq!(fixture.run(&["start", "demo"]).status.code(), Some(1));
}

#[test]
fn node_shims_and_manager_proxies_are_not_executed() {
    let fixture = Fixture::new();
    let shim = fixture.root.join("shims/node");
    fs::create_dir(shim.parent().unwrap()).unwrap();
    fs::write(
        &shim,
        format!(
            "#!/bin/sh\nprintf called > '{}'\n",
            fixture.root.join("shim-called").display()
        ),
    )
    .unwrap();
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o700)).unwrap();
    let output = fixture.run(&["doctor", "--node", shim.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!fixture.root.join("shim-called").exists());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("unknown")
    );
    let package = fixture
        .node
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("lib/node_modules/npm/package.json");
    fs::write(
        package,
        json!({"name":"corepack","version":"0.34.0","bin":{"npm":"bin/npm-cli.js"}}).to_string(),
    )
    .unwrap();
    let output = fixture.run(&["doctor", "--node", fixture.node.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("shims were not executed")
    );
}

#[test]
fn stalled_and_flooding_native_probes_are_bounded_and_cleanup_descendants() {
    let fixture = Fixture::new();
    let tools = fixture.node.parent().unwrap().parent().unwrap();
    fs::write(tools.join("mode"), "stall").unwrap();
    let start = Instant::now();
    let output = fixture.run(&["doctor", "--node", fixture.node.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(start.elapsed() < Duration::from_secs(7));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("timed out")
    );
    for name in ["probe-parent", "probe-child"] {
        let pid: i32 = fs::read_to_string(tools.join(name))
            .unwrap()
            .parse()
            .unwrap();
        let out = Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .unwrap();
        let state = String::from_utf8_lossy(&out.stdout);
        assert!(
            state.trim().is_empty() || state.starts_with('Z'),
            "{name}: {state}"
        );
    }
    fs::write(tools.join("mode"), "flood").unwrap();
    let output = fixture.run(&["doctor", "--node", fixture.node.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("output limit")
    );
}

#[test]
fn changed_bindings_fail_without_path_fallback_and_malformed_storage_is_rejected() {
    let fixture = Fixture::new();
    fixture.adopt();
    fs::write(
        fixture
            .node
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("version"),
        "22.0.0",
    )
    .unwrap();
    let output = fixture.run(&["start", "demo"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("changed version")
    );
    assert!(!fixture.project.join("observed").exists());
    let dir = fs::read_dir(fixture.binding_dir())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_type().unwrap().is_dir())
        .unwrap()
        .path();
    fs::write(dir.join("bin/npm"), "#!/bin/sh\nexit 0\n").unwrap();
    let output = fixture.run(&["doctor"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("launcher changed")
    );
}

#[test]
fn managed_services_use_independent_versions_and_batch_failure_starts_none() {
    let fixture = Fixture::new();
    fixture.adopt();
    let second = fixture.root.join("second");
    fs::create_dir_all(second.join(".git")).unwrap();
    fs::write(
        second.join("package.json"),
        r#"{"scripts":{"dev":"fixture"}}"#,
    )
    .unwrap();
    let second_node = Fixture::toolchain(&fixture.root.join("other-tools"), "22.0.0", "10.0.0");
    let mut config = fs::read_to_string(&fixture.config).unwrap();
    config.push_str(&format!("\n[node_environments.other]\nversion='22.0.0'\npackage_manager='npm'\npackage_manager_version='10.0.0'\n[projects.second]\npath='{}'\n[services.second]\nproject='second'\ncmd=['npm','run','dev']\nnode_environment='other'\n", second.display()));
    fs::write(&fixture.config, config).unwrap();
    let output = fixture.run(&["start", "--all"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!fixture.project.join("observed").exists());
    let output = fixture.run(&[
        "setup",
        "second",
        "--node",
        second_node.to_str().unwrap(),
        "--apply",
        "--yes",
    ]);
    assert!(output.status.success(), "{output:?}");
    let output = fixture.run(&["start", "--all"]);
    assert!(output.status.success(), "{output:?}");
    assert!(fixture.observed(&fixture.project).contains("node=v24.0.0"));
    assert!(fixture.observed(&second).contains("node=v22.0.0"));
}

#[test]
fn status_and_stop_remain_responsive_and_can_cancel_a_managed_preflight() {
    let fixture = Fixture::new();
    fixture.adopt();
    assert!(fixture.run(&["start", "demo"]).status.success());
    fixture.observed(&fixture.project);
    assert!(fixture.run(&["stop", "demo"]).status.success());
    fs::remove_file(fixture.project.join("observed")).unwrap();
    let tools = fixture.node.parent().unwrap().parent().unwrap();
    fs::write(tools.join("mode"), "stall").unwrap();
    let child = fixture
        .command(&["start", "demo"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !tools.join("probe-parent").exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let start = Instant::now();
    assert!(fixture.run(&["status"]).status.success());
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(fixture.run(&["stop", "demo"]).status.success());
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("cancelled")
    );
    assert!(!fixture.project.join("observed").exists());
}

#[test]
fn pnpm_and_nested_npm_are_bound_without_executing_corepack_proxies() {
    let fixture = Fixture::new();
    let tools = fixture.node.parent().unwrap().parent().unwrap();
    let package = tools.join("lib/node_modules/pnpm");
    fs::create_dir_all(package.join("bin")).unwrap();
    fs::write(package.join("package.json"), json!({"name":"pnpm","version":"10.3.0","engines":{"node":">=18"},"bin":{"pnpm":"bin/pnpm.cjs"}}).to_string()).unwrap();
    fs::write(package.join("bin/pnpm.cjs"), "10.3.0").unwrap();
    let corepack = tools.join("lib/node_modules/corepack");
    fs::create_dir_all(corepack.join("dist")).unwrap();
    fs::write(
        corepack.join("package.json"),
        json!({"name":"corepack","version":"0.34.0","bin":{"pnpm":"dist/pnpm.js"}}).to_string(),
    )
    .unwrap();
    fs::write(corepack.join("dist/pnpm.js"), "not-an-actual-manager").unwrap();
    symlink(corepack.join("dist/pnpm.js"), tools.join("bin/pnpm")).unwrap();
    let config = fs::read_to_string(&fixture.config)
        .unwrap()
        .replace("package_manager = 'npm'", "package_manager = 'pnpm'")
        .replace(
            "package_manager_version = '11.0.0'",
            "package_manager_version = '10.3.0'",
        )
        .replace("cmd = ['npm'", "cmd = ['pnpm'");
    fs::write(&fixture.config, config).unwrap();
    fixture.adopt();
    assert!(fixture.run(&["start", "demo"]).status.success());
    assert_eq!(
        fixture.observed(&fixture.project),
        "node=v24.0.0\nmanager=10.3.0\nnpm=11.0.0\n"
    );
}

#[test]
fn explicit_same_path_refresh_is_atomic_and_cannot_change_a_live_binding() {
    let fixture = Fixture::new();
    fixture.adopt();
    assert!(fixture.run(&["start", "demo"]).status.success());
    fixture.observed(&fixture.project);
    let result = fixture.run(&[
        "setup",
        "--node",
        fixture.node.to_str().unwrap(),
        "--apply",
        "--yes",
    ]);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8(result.stderr).unwrap().contains("busy"));
    assert!(fixture.run(&["stop", "demo"]).status.success());
    fs::File::open(&fixture.node)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(1))
        .unwrap();
    let result = fixture.run(&["start", "demo"]);
    assert_eq!(result.status.code(), Some(1));
    let preview = fixture.run(&["setup", "--node", fixture.node.to_str().unwrap()]);
    assert!(preview.status.success());
    assert!(
        String::from_utf8(preview.stdout)
            .unwrap()
            .contains("metadata changed")
    );
    fixture.adopt();
    assert!(fixture.run(&["doctor"]).status.success());
    assert!(fixture.run(&["start", "demo"]).status.success());
}

#[test]
fn managed_configuration_rejects_unknown_fields_references_and_arbitrary_commands() {
    let fixture = Fixture::new();
    let original = fs::read_to_string(&fixture.config).unwrap();
    for changed in [
        original.replace("node_environment = 'local'", "node_environment = 'absent'"),
        original.replace("version = '24.0.0'", "version = '^24'"),
        original.replace(
            "cmd = ['npm', 'run', 'dev']",
            "cmd = ['sh', '-c', 'npm run dev']",
        ),
        format!("{original}\nmanaged = {{}}\n"),
        format!("{original}\nlaunch = {{}}\n"),
    ] {
        fs::write(&fixture.config, changed).unwrap();
        let result = fixture.run(&["validate"]);
        assert_eq!(result.status.code(), Some(1), "{result:?}");
    }
    fs::write(&fixture.config, original).unwrap();
    assert!(fixture.run(&["validate"]).status.success());
}

#[test]
fn project_bin_entries_cannot_shadow_bound_tools_and_launcher_paths_are_quoted() {
    let mut fixture = Fixture::new();
    let special = fixture
        .root
        .join("tools with space ' $(touch SHOULD_NOT_EXIST)");
    fs::rename(fixture.root.join("tools"), &special).unwrap();
    fixture.node = special.join("bin/node");
    // Keep the npm entry symlink valid after moving the installation.
    fs::remove_file(special.join("bin/npm")).unwrap();
    symlink(
        special.join("lib/node_modules/npm/bin/npm-cli.js"),
        special.join("bin/npm"),
    )
    .unwrap();
    let bins = fixture.project.join("node_modules/.bin");
    fs::create_dir_all(&bins).unwrap();
    for name in ["node", "npm", "pnpm"] {
        let path = bins.join(name);
        fs::write(&path, "#!/bin/sh\ntouch SHADOW_EXECUTED\nexit 89\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fixture.adopt();
    assert!(fixture.run(&["start", "demo"]).status.success());
    assert_eq!(
        fixture.observed(&fixture.project),
        "node=v24.0.0\nmanager=11.0.0\nnpm=11.0.0\n"
    );
    assert!(!fixture.project.join("SHADOW_EXECUTED").exists());
    assert!(!fixture.project.join("SHOULD_NOT_EXIST").exists());
}

#[test]
fn unsupported_script_shell_and_external_project_settings_block_without_exposing_values() {
    let fixture = Fixture::new();
    fixture.adopt();
    let external = fixture.root.join("external-npmrc");
    fs::write(&external, "script-shell=PRIVATE_SETTING\n").unwrap();
    let output = fixture
        .command(&["doctor"])
        .env("NpM_cOnFiG_UsErCoNfIg", &external)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("PRIVATE_SETTING"));
    fs::write(
        fixture.project.join(".npmrc"),
        "script-shell=PRIVATE_SETTING\n",
    )
    .unwrap();
    let output = fixture.run(&["start", "demo"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("PRIVATE_SETTING"));
    assert!(!fixture.project.join("observed").exists());
    fs::write(
        fixture.project.join(".npmrc"),
        "\"script\\u002dshell\"=PRIVATE_SETTING\n",
    )
    .unwrap();
    let output = fixture.run(&["doctor"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("Unsupported runtime/hook key script-shell")
    );
    fs::remove_file(fixture.project.join(".npmrc")).unwrap();
    fs::write(
        fixture.root.join("outside-npmrc"),
        "registry=https://example.invalid\n",
    )
    .unwrap();
    symlink(
        fixture.root.join("outside-npmrc"),
        fixture.project.join(".npmrc"),
    )
    .unwrap();
    let output = fixture.run(&["doctor"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("escape"));
}

#[test]
fn interrupting_doctor_terminates_its_native_probe_and_descendant() {
    let fixture = Fixture::new();
    let tools = fixture.node.parent().unwrap().parent().unwrap();
    fs::write(tools.join("mode"), "stall").unwrap();
    let child = fixture
        .command(&["doctor", "--node", fixture.node.to_str().unwrap()])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !tools.join("probe-child").exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(child.id() as i32),
        nix::sys::signal::Signal::SIGINT,
    )
    .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cancelled"));
    for name in ["probe-parent", "probe-child"] {
        let pid = fs::read_to_string(tools.join(name)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let out = Command::new("ps")
                .args(["-o", "stat=", "-p", pid.trim()])
                .output()
                .unwrap();
            let state = String::from_utf8_lossy(&out.stdout);
            if state.trim().is_empty() || state.starts_with('Z') {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "probe survived cancellation: {state}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    assert!(fs::read_dir(&fixture.home).unwrap().next().is_none());
}

#[test]
fn real_terminal_reviews_bindings_and_requires_fresh_visible_start_confirmation() {
    let fixture = Fixture::new();
    for mode in ["cancel", "changed", "apply", "tui"] {
        if mode == "apply" {
            fixture.manifest(json!({"scripts":{"dev":"fixture"}}));
        }
        if mode == "tui" {
            fixture
                .manifest(json!({"scripts":{"dev":"fixture"},"dependencies":{"example":"1.0.0"}}));
            fs::create_dir(fixture.project.join("node_modules")).unwrap();
        }
        let command = fixture.command(&[]);
        // Use the fixture's isolated environment for the Python-driven real PTY.
        let environment: Vec<_> = command
            .get_envs()
            .map(|(key, value)| (key.to_os_string(), value.map(std::ffi::OsStr::to_os_string)))
            .collect();
        let mut python = Command::new("python3");
        for (key, value) in environment {
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
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/environment_pty.py"))
            .args([
                env!("CARGO_BIN_EXE_devctl"),
                fixture.config.to_str().unwrap(),
                fixture.node.to_str().unwrap(),
                mode,
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "mode={mode}: {output:?}");
    }
}
