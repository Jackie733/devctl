use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub struct ConfigFixture {
    dir: PathBuf,
    pub path: PathBuf,
}

impl ConfigFixture {
    pub fn new(example: &str) -> Self {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(example);
        let dir = temp_root().join(format!(
            "devctl-config-fixture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        let mut value: toml::Value = toml::from_str(&fs::read_to_string(&source).unwrap()).unwrap();
        for (_, project) in value["projects"].as_table_mut().unwrap().iter_mut() {
            let resolved = source
                .parent()
                .unwrap()
                .join(project["path"].as_str().unwrap())
                .canonicalize()
                .unwrap();
            project["path"] = toml::Value::String(resolved.to_str().unwrap().into());
        }
        value.as_table_mut().unwrap().insert(
            "logs".into(),
            toml::Value::Table(toml::Table::from_iter([(
                "enabled".into(),
                toml::Value::Boolean(false),
            )])),
        );
        let path = dir.join("devctl.toml");
        fs::write(&path, toml::to_string(&value).unwrap()).unwrap();
        Self { dir, path }
    }
}

impl Drop for ConfigFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[allow(dead_code)]
pub fn process_is_live(pid: nix::unistd::Pid) -> bool {
    let output = std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .expect("ps must run");
    let state = String::from_utf8(output.stdout).unwrap();
    !state.trim().is_empty() && !state.trim().starts_with('Z') && !state.trim().starts_with('X')
}

#[allow(dead_code)]
pub fn temp_root() -> PathBuf {
    // Darwin's per-user TMPDIR can exceed the Unix socket path budget once the
    // fixture suffix is added. Keep control-socket fixtures short on that host.
    if cfg!(target_os = "macos") {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    }
}
