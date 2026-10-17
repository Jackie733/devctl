mod archive;
mod cli;
mod client;
mod config;
mod control;
mod control_socket;
mod discovery;
mod environment;
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
mod update;

use std::{path::Path, process::ExitCode};

use clap::Parser;
use cli::{Cli, Command};
use manager::Manager;
use tokio::signal::unix::{SignalKind, signal};

fn main() -> ExitCode {
    let cli = Cli::parse();
    #[cfg(target_os = "macos")]
    if matches!(
        cli.command.as_ref(),
        Some(Command::Supervise { detached: true })
    ) && let Err(error) = startup::close_inherited_descriptors()
    {
        startup::failed(&error);
        eprintln!("Error: {error}");
        return ExitCode::FAILURE;
    }
    run(cli)
}

#[tokio::main(flavor = "current_thread")]
async fn run(cli: Cli) -> ExitCode {
    match execute(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn execute(cli: Cli) -> Result<(), String> {
    let command = cli.command.unwrap_or(Command::Tui);
    if let Command::Import { directory } = &command {
        if cli.config.is_some() {
            return Err("The read-only import preview does not select a workspace yet; omit --config. Configuration saving is not implemented.".into());
        }
        return discovery::preview(directory);
    }
    if let Command::Update {
        check,
        version,
        rollback,
    } = command
    {
        return update::run(check, version.as_deref(), rollback).await;
    }
    let config_path = config::resolve_path(cli.config)?;
    match command {
        Command::Import { .. } => {
            unreachable!("Import preview is handled before config discovery.")
        }
        Command::Update { .. } => unreachable!("Updates are handled before config discovery."),
        Command::Tui => tui::run(&config_path).await,
        Command::Supervise { detached } => supervisor::run(&config_path, detached).await,
        Command::Doctor { services, node } => {
            environment::interruptible(environment::doctor(
                &config::load(&config_path)?,
                &services,
                node.as_deref(),
            ))
            .await
        }
        Command::Setup {
            services,
            node,
            apply,
            yes,
        } => {
            environment::interruptible(environment::setup(
                &config::load(&config_path)?,
                &services,
                node.as_deref(),
                apply,
                yes,
            ))
            .await
        }
        Command::Start {
            services,
            all,
            allow_unverified_dependencies,
        } => {
            client::execute(
                &config_path,
                &control::Request::Start {
                    targets: services,
                    all,
                    allow_unverified_dependencies,
                },
                true,
            )
            .await?;
            println!("Start request accepted; query status for process state.");
            Ok(())
        }
        Command::Stop { services, all } => {
            client::execute(
                &config_path,
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
        Command::Status => {
            let response =
                client::execute(&config_path, &control::Request::Status {}, false).await?;
            for (id, status) in response.services {
                println!("{id}: {:?}", status.state);
                if let Some(error) = status.error {
                    println!("  {error}");
                }
            }
            Ok(())
        }
        Command::LogSessions => {
            let response =
                client::read_logs(&config_path, control::Request::LogSessions {}).await?;
            for session in response.sessions {
                println!(
                    "{} created_ms={} {} services={}",
                    session.id,
                    session.created_ms,
                    if session.complete {
                        "closed"
                    } else {
                        "unclosed"
                    },
                    session
                        .services
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
            Ok(())
        }
        Command::Logs {
            service,
            limit,
            after,
            before,
            session,
            contains,
            upper,
        } => {
            let searching = contains.is_some();
            let response = client::read_logs(
                &config_path,
                control::Request::ReadLogs {
                    query: archive::Query {
                        service: service.clone(),
                        limit: usize::from(limit),
                        after,
                        before,
                        session,
                        contains,
                        upper,
                    },
                },
            )
            .await?;
            let logs = response
                .logs
                .ok_or_else(|| "Missing log response.".to_owned())?;
            if let Some(info) = &logs.storage {
                eprintln!(
                    "Session: {} | storage={} | {}",
                    info.session, info.state, info.directory
                );
                if !info.warning.is_empty() {
                    eprintln!("Warning: {}", info.warning);
                }
            }
            if searching {
                eprintln!(
                    "Search upper={} complete={} scanned_to={}",
                    upper.unwrap_or(logs.latest),
                    logs.search_complete,
                    logs.scanned_to.unwrap_or(0)
                );
                if !logs.search_complete {
                    eprintln!(
                        "Continue with --{} {} --upper {}.",
                        if after.is_some() { "after" } else { "before" },
                        logs.scanned_to.unwrap_or(0),
                        upper.unwrap_or(logs.latest)
                    );
                }
            }
            if logs.dropped > 0 {
                eprintln!(
                    "Warning: {} log records were dropped or evicted.",
                    logs.dropped
                );
            }
            if logs.incomplete {
                eprintln!("Warning: captured or saved log output may be incomplete.");
            }
            if logs.gap {
                eprintln!("Warning: requested log cursor precedes retained output.");
            }
            if logs.has_older {
                if let Some(first) = logs.records.first() {
                    eprintln!("Earlier retained output: use --before {}.", first.sequence);
                }
            } else if before.is_some() && logs.oldest.is_some() {
                eprintln!("Reached the start of retained output.");
            }
            for record in logs.records {
                let source = match record.source {
                    logs::Source::Stdout => "stdout",
                    logs::Source::Stderr => "stderr",
                };
                let marker = if record.truncated { " [truncated]" } else { "" };
                println!(
                    "[{service}#{} {source} utc_ms={} seq={}] {}{marker}",
                    record.instance, record.captured_ms, record.sequence, record.text
                );
            }
            Ok(())
        }
        Command::Shutdown => {
            client::execute(&config_path, &control::Request::Shutdown {}, false).await?;
            println!("Workspace shut down.");
            Ok(())
        }
        Command::Validate => print_validation(&config_path),
        Command::Plan { targets } => print_plan(&config_path, &targets),
        Command::Run {
            services,
            all,
            allow_unverified_dependencies,
        } => run_services(&config_path, &services, all, allow_unverified_dependencies).await,
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

async fn run_services(
    path: &Path,
    targets: &[String],
    all: bool,
    allow_unverified: bool,
) -> Result<(), String> {
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
    let sources = manager.start_sources(&selected)?;
    let prepared = if sources.values().any(|service| service.managed.is_some()) {
        tokio::select! {
            prepared = environment::prepare(sources, allow_unverified) => prepared?,
            _ = interrupt.recv() => return Err("Environment checks cancelled; no services were started.".into()),
            _ = terminate.recv() => return Err("Environment checks cancelled; no services were started.".into()),
        }
    } else {
        sources
    };
    let started = manager.start_prepared(prepared);
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
