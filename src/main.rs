mod cli;
mod config;
mod manager;
mod plan;
mod process;
mod service_task;

use std::{collections::BTreeSet, path::Path, process::ExitCode};

use clap::{CommandFactory, Parser};
use cli::{Cli, Command};
use manager::Manager;
use tokio::signal::unix::{SignalKind, signal};

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match execute(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn execute(cli: Cli) -> Result<(), String> {
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
        Some(Command::Run { services }) => run_services(&cli.config, &services).await,
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

async fn run_services(path: &Path, targets: &[String]) -> Result<(), String> {
    let mut config = config::load(path)?;
    let mut seen = BTreeSet::new();
    let mut selected = Vec::new();

    for id in targets {
        if !seen.insert(id) {
            continue;
        }
        let service = config
            .services
            .remove(id)
            .ok_or_else(|| format!("Unknown service '{id}'."))?;
        if !service.depends_on.is_empty() {
            return Err(format!(
                "Service '{id}' has dependencies. Dependency execution is not supported yet."
            ));
        }
        selected.push((id.clone(), service));
    }

    let mut interrupt = signal(SignalKind::interrupt())
        .map_err(|error| format!("Failed to register SIGINT handler: {error}"))?;
    let mut terminate = signal(SignalKind::terminate())
        .map_err(|error| format!("Failed to register SIGTERM handler: {error}"))?;

    let mut manager = Manager::default();
    for (id, service) in selected {
        if let Err(error) = manager.start(id, service) {
            return match manager.stop_all().await {
                Ok(()) => Err(error),
                Err(cleanup) => Err(format!("{error}\n{cleanup}")),
            };
        }
    }

    let mut failed = false;
    let stop_requested = loop {
        let completion = tokio::select! {
            completion = manager.wait_next() => completion,
            Some(()) = interrupt.recv() => break true,
            Some(()) = terminate.recv() => break true,
        };
        let Some((id, result)) = completion else {
            break false;
        };

        let error = match result {
            Ok(process::RunOutcome::Exited(status)) => {
                Some(format!("Service '{id}' exited unexpectedly ({status})."))
            }
            Ok(process::RunOutcome::Stopped) => None,
            Err(error) => Some(format!("Service '{id}': {error}")),
        };
        if let Some(error) = error {
            eprintln!("Error: {error}");
            failed = true;
        }
    };

    if stop_requested && let Err(error) = manager.stop_all().await {
        eprintln!("Error: {error}");
        failed = true;
    }

    if failed {
        Err("One or more services failed.".to_owned())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        sync::oneshot,
        time::{Duration, timeout},
    };

    #[tokio::test]
    async fn a_stop_queued_before_the_task_runs_is_honored() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/process.toml");
        let service = config::load(&path)
            .unwrap()
            .services
            .remove("long_running")
            .unwrap();
        let (sender, receiver) = oneshot::channel();
        let task =
            tokio::spawn(async move { process::run("long_running", &service, receiver).await });

        sender.send(()).unwrap();
        let outcome = timeout(Duration::from_secs(8), task)
            .await
            .expect("The queued stop must finish within the cleanup budget.")
            .unwrap()
            .unwrap();
        assert!(matches!(outcome, process::RunOutcome::Stopped));
    }
}
