//! Native probe/lifecycle fixture. Compiled by environment_cli, never distributed.
use std::{env, fs, io::Write, process::Command, thread, time::Duration};

fn main() {
    let executable = env::current_exe().unwrap().canonicalize().unwrap();
    let root = executable.parent().unwrap().parent().unwrap();
    let arguments: Vec<_> = env::args().skip(1).collect();
    if arguments == ["--version"] {
        let mode = fs::read_to_string(root.join("mode")).unwrap_or_default();
        if mode == "stall" {
            let child = Command::new("/bin/sh")
                .args(["-c", "sleep 60"])
                .spawn()
                .unwrap();
            fs::write(root.join("probe-child"), child.id().to_string()).unwrap();
            fs::write(root.join("probe-parent"), std::process::id().to_string()).unwrap();
            thread::sleep(Duration::from_secs(60));
        }
        if mode == "flood" {
            std::io::stdout()
                .write_all(&vec![b'x'; 128 * 1024])
                .unwrap();
            thread::sleep(Duration::from_secs(60));
        }
        println!(
            "v{}",
            fs::read_to_string(root.join("version")).unwrap().trim()
        );
        return;
    }
    if arguments.len() == 2 && arguments[1] == "--version" {
        println!("{}", fs::read_to_string(&arguments[0]).unwrap().trim());
        return;
    }
    if arguments.len() == 3 && arguments[1] == "run" {
        let manager = if arguments[0].contains("pnpm") {
            "pnpm"
        } else {
            "npm"
        };
        let cwd = env::current_dir().unwrap();
        let mut paths = vec![cwd.join("node_modules/.bin")];
        paths.extend(env::split_paths(&env::var_os("PATH").unwrap()));
        let status = Command::new(
            env::var_os("npm_config_script_shell").unwrap_or_else(|| "/bin/sh".into()),
        )
        .args(["-c", &format!("exec node --record {manager}")])
        .env("PATH", env::join_paths(paths).unwrap())
        .status()
        .unwrap();
        std::process::exit(status.code().unwrap_or(1));
    }
    if arguments.len() == 2 && arguments[0] == "--record" {
        let node = Command::new("node").arg("--version").output().unwrap();
        let manager = &arguments[1];
        let tool = Command::new(manager).arg("--version").output().unwrap();
        let npm = Command::new("npm").arg("--version").output().unwrap();
        let cwd = env::current_dir().unwrap();
        fs::write(
            cwd.join("observed"),
            format!(
                "node={}\nmanager={}\nnpm={}\n",
                String::from_utf8_lossy(&node.stdout).trim(),
                String::from_utf8_lossy(&tool.stdout).trim(),
                String::from_utf8_lossy(&npm.stdout).trim()
            ),
        )
        .unwrap();
        println!("fixture service started");
        thread::sleep(Duration::from_secs(60));
        return;
    }
    std::process::exit(19);
}
