use std::{
    ffi::OsString,
    path::Path,
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use nix::{
    sys::signal::{Signal, killpg},
    unistd::Pid,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::{Child, Command},
    time::timeout,
};

use crate::process;

const OUTPUT_LIMIT: usize = 64 * 1024;

struct Guard {
    child: Child,
    pgid: Pid,
    armed: bool,
}

impl Guard {
    async fn finish(&mut self) -> Result<std::process::ExitStatus, String> {
        let result = process::kill_and_reap("environment probe", &mut self.child, self.pgid).await;
        if result.is_ok() {
            self.armed = false;
        }
        result
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if self.armed {
            // Kill descendants before Child's drop can reap and release the leader PID.
            let _ = killpg(self.pgid, Signal::SIGKILL);
            let _ = self.child.start_kill();
        }
    }
}

async fn read(
    mut stream: impl AsyncRead + Unpin,
    used: Arc<AtomicUsize>,
) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut chunk = [0; 4096];
    loop {
        let count = stream
            .read(&mut chunk)
            .await
            .map_err(|error| format!("Cannot read environment probe output: {error}"))?;
        if count == 0 {
            return Ok(output);
        }
        if used.fetch_add(count, Ordering::Relaxed) + count > OUTPUT_LIMIT {
            return Err("Environment probe exceeded its output limit.".into());
        }
        output.extend_from_slice(&chunk[..count]);
    }
}

/// Trusted tool entry points only. No project cwd, inherited Node preloads or npmrc.
pub(super) async fn run(node: &Path, arguments: &[OsString]) -> Result<String, String> {
    const GLOBAL_CONFIG: &str = "/nonexistent/devctl-global-npmrc";
    if std::fs::symlink_metadata(GLOBAL_CONFIG).is_ok() {
        return Err(
            "Neutral npm configuration path unexpectedly exists; probe was not executed.".into(),
        );
    }
    let parent = node
        .parent()
        .ok_or("Node executable has no containing directory.")?;
    let path = std::env::join_paths([parent, Path::new("/usr/bin"), Path::new("/bin")])
        .map_err(|error| error.to_string())?;
    let child = Command::new(node)
        .args(arguments)
        .current_dir("/")
        .env_clear()
        .env("PATH", path)
        .env("HOME", "/")
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .env("NO_UPDATE_NOTIFIER", "1")
        .env("npm_config_userconfig", "/dev/null")
        .env("npm_config_globalconfig", GLOBAL_CONFIG)
        .env("npm_config_cache", "/dev/null")
        .env("npm_config_logs_max", "0")
        .env("npm_config_update_notifier", "false")
        .env("npm_config_offline", "true")
        .env("npm_config_ignore_scripts", "true")
        .env("npm_config_ignore_pnpmfile", "true")
        .env("npm_config_manage_package_manager_versions", "false")
        .env("COREPACK_ENABLE_NETWORK", "0")
        .env("XDG_CONFIG_HOME", "/dev/null")
        .env("PNPM_HOME", "/dev/null")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("Cannot execute environment probe: {error}"))?;
    let pgid = Pid::from_raw(
        i32::try_from(child.id().ok_or("Environment probe has no PID.")?)
            .map_err(|error| error.to_string())?,
    );
    let mut guard = Guard {
        child,
        pgid,
        armed: true,
    };
    let stdout = guard.child.stdout.take().expect("stdout was piped");
    let stderr = guard.child.stderr.take().expect("stderr was piped");
    let used = Arc::new(AtomicUsize::new(0));
    let result = timeout(Duration::from_secs(3), async {
        let exit = async {
            process::wait_for_exit_without_reaping("environment probe", pgid).await?;
            guard.finish().await
        };
        tokio::try_join!(read(stdout, used.clone()), read(stderr, used), exit)
    })
    .await;
    match result {
        Ok(Ok((stdout, _stderr, status))) if status.success() => String::from_utf8(stdout)
            .map(|text| text.trim().into())
            .map_err(|_| "Environment probe returned invalid UTF-8.".into()),
        result => {
            if guard.armed {
                guard.finish().await?;
            }
            match result {
                Err(_) => Err("Environment probe timed out after 3 seconds.".into()),
                Ok(Err(error)) => Err(error),
                Ok(Ok((_, _, status))) => Err(format!(
                    "Environment probe failed ({status}); run the tool separately for its private diagnostics."
                )),
            }
        }
    }
}
