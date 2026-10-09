mod cli;
mod config;
mod plan;
mod process;

use std::{path::Path, process::ExitCode};

use clap::{CommandFactory, Parser};
use cli::{Cli, Command};

fn main() -> ExitCode {
    match execute(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute(cli: Cli) -> Result<(), String> {
    match cli.command {
        None => {
            Cli::command()
                .print_help()
                .map_err(|error| format!("Failed to print help: {error}"))?;
            println!();
            Ok(())
        }
        Some(Command::Validate) => print_validation(&cli.config),
        Some(Command::Plan { targets }) => print_plan(&cli.config, &targets),
        Some(Command::Run { service }) => run_service(&cli.config, &service),
    }
}

fn print_validation(path: &Path) -> Result<(), String> {
    let config = config::load(path)?;

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

    Ok(())
}

fn print_plan(path: &Path, targets: &[String]) -> Result<(), String> {
    let config = config::load(path)?;
    let order = plan::build(&config, targets)?;

    if order.is_empty() {
        println!("No services selected.");
        return Ok(());
    }

    println!("Startup plan: {}", order.join(" -> "));

    for id in &order {
        let service = config
            .services
            .get(id)
            .ok_or_else(|| format!("Unknown service '{id}' in startup plan."))?;

        println!("Service: {id}");
        println!("  Command: {:?}", service.cmd);

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

    Ok(())
}

fn run_service(path: &Path, id: &str) -> Result<(), String> {
    let config = config::load(path)?;

    let service = config
        .services
        .get(id)
        .ok_or_else(|| format!("Unknown service '{id}'."))?;

    if !service.depends_on.is_empty() {
        return Err(format!(
            "Service '{id}' has dependencies. Dependency execution is not supported yet."
        ));
    }

    let status = process::run(id, service)?;

    Err(format!("Service '{id}' exited unexpectedly ({status})."))
}
