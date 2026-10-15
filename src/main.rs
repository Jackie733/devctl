mod cli;
mod client;
mod config;
mod control;
mod control_socket;
mod logs;
mod manager;
mod plan;
mod probe;
mod process;
mod run_lock;
mod service_task;
mod startup;
mod supervisor;
mod tui;

use std::{path::Path, process::ExitCode};

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
        Some(Command::Tui) => tui::run().await,
        Some(Command::Supervise { detached }) => supervisor::run(&cli.config, detached).await,
        Some(Command::Start { services, all }) => {
            client::execute(
                &cli.config,
                &control::Request::Start {
                    targets: services,
                    all,
                },
                true,
            )
            .await?;
            println!("Start request accepted; query status for process state.");
            Ok(())
        }
        Some(Command::Stop { services, all }) => {
            client::execute(
                &cli.config,
                &control::Request::Stop {
                    targets: services,
                    all,
                },
                false,
            )
            .await?;
            println!("Selected services stopped.");
            Ok(())
        }
        Some(Command::Status) => {
            let response =
                client::execute(&cli.config, &control::Request::Status {}, false).await?;
            for (id, status) in response.services {
                println!("{id}: {:?}", status.state);
                if let Some(error) = status.error {
                    println!("  {error}");
                }
            }
            Ok(())
        }
        Some(Command::Logs {
            service,
            limit,
            after,
        }) => {
            let response = client::execute(
                &cli.config,
                &control::Request::Logs {
                    service: service.clone(),
                    limit: usize::from(limit),
                    after,
                },
                false,
            )
            .await?;
            let logs = response
                .logs
                .ok_or_else(|| "Missing log response.".to_owned())?;
            if logs.dropped > 0 {
                eprintln!(
                    "Warning: {} log records were dropped or evicted.",
                    logs.dropped
                );
            }
            if logs.incomplete {
                eprintln!("Warning: log capture ended early; output may be incomplete.");
            }
            if logs.gap {
                eprintln!("Warning: requested log cursor precedes retained output.");
            }
            for record in logs.records {
                let source = match record.source {
                    logs::Source::Stdout => "stdout",
                    logs::Source::Stderr => "stderr",
                };
                let marker = if record.truncated { " [truncated]" } else { "" };
                println!(
                    "[{service}#{} {source} seq={}] {}{marker}",
                    record.instance, record.sequence, record.text
                );
            }
            Ok(())
        }
        Some(Command::Shutdown) => {
            client::execute(&cli.config, &control::Request::Shutdown {}, false).await?;
            println!("Workspace shut down.");
            Ok(())
        }
        Some(Command::Validate) => print_validation(&cli.config),
        Some(Command::Plan { targets }) => print_plan(&cli.config, &targets),
        Some(Command::Run { services, all }) => run_services(&cli.config, &services, all).await,
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

async fn run_services(path: &Path, targets: &[String], all: bool) -> Result<(), String> {
    let path = path
        .canonicalize()
        .map_err(|error| format!("Failed to resolve config path {}: {error}", path.display()))?;
    let config = config::load(&path)?;
    let mut manager = Manager::new(config.services);
    let selected = if all {
        manager.default_targets()
    } else {
        targets.to_vec()
    };
    manager.check_start(&selected)?;
    if selected.is_empty() {
        println!("No services selected.");
        return Ok(());
    }

    let mut interrupt = signal(SignalKind::interrupt())
        .map_err(|error| format!("Failed to register SIGINT handler: {error}"))?;
    let mut terminate = signal(SignalKind::terminate())
        .map_err(|error| format!("Failed to register SIGTERM handler: {error}"))?;

    let _run_lock = run_lock::RunLock::acquire(&path)?;
    let started = if all {
        manager.start_all()
    } else {
        manager.start_selected(&selected)
    };
    if let Err(error) = started {
        return match manager.stop_all().await {
            Ok(()) => Err(error),
            Err(cleanup) => Err(format!("{error}\n{cleanup}")),
        };
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
        let task = tokio::spawn(async move {
            process::run("long_running", &service, receiver, None, None).await
        });

        sender.send(()).unwrap();
        let outcome = timeout(Duration::from_secs(8), task)
            .await
            .expect("The queued stop must finish within the cleanup budget.")
            .unwrap()
            .unwrap();
        assert!(matches!(outcome, process::RunOutcome::Stopped));
    }
}
