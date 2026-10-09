mod config;

use std::{env, path::Path, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.as_slice() {
        [] => {
            print_help();
            ExitCode::SUCCESS
        }
        [arg] if arg == "--help" || arg == "-h" => {
            print_help();
            ExitCode::SUCCESS
        }
        [arg] if arg == "--version" || arg == "-V" => {
            println!("devctl {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [flag, path, command] if flag == "--config" && command == "validate" => {
            match config::load(Path::new(path)) {
                Ok(config) => {
                    println!("Configuration is valid.");
                    println!("Workspace: {}", config.workspace.name);
                    println!("Projects: {}", config.projects.len());
                    println!("Services: {}", config.services.len());
                    for (id, project) in &config.projects {
                        println!("Project: {id}, path: {}", project.path.display());
                    }
                    for (id, service) in &config.services {
                        println!(
                            "Service: {id}, project: {}, autostart: {}",
                            service.project, service.autostart
                        );
                        if let Some(cwd) = &service.cwd {
                            println!("  Working directory: {}", cwd.display());
                        }

                        for dependency in &service.depends_on {
                            println!(
                                "  Depends on: {}, condition: {:?}",
                                dependency.service, dependency.condition
                            );
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("Error: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("Error: unsupported arguments. Use --help for usage.");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!(
        "devctl - Development workspace manager
Usage:
    devctl [OPTIONS]
    devctl --config <PATH> validate

Options:
    -h, --help      Show help
    -V, --version   Show version

Commands:
    validate        Validate workspace, project, and service definitions"
    );
}
