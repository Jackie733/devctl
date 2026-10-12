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
        let dir = std::env::temp_dir().join(format!(
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
