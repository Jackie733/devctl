use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about = "Development workspace manager")]
pub struct Cli {
    /// Config path; otherwise search ./devctl.toml, ~/devctl.toml, ~/.config/devctl/devctl.toml.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Open the terminal interface.
    Tui,

    /// Internal supervisor entry point.
    #[command(hide = true)]
    Supervise {
        #[arg(long, hide = true)]
        detached: bool,
    },

    /// Start selected services in a background workspace.
    Start {
        #[arg(value_name = "SERVICE", required_unless_present = "all", num_args = 1.., conflicts_with = "all")]
        services: Vec<String>,
        #[arg(long)]
        all: bool,
    },
    /// Stop selected services, keeping the workspace available.
    Stop {
        #[arg(value_name = "SERVICE", required_unless_present = "all", num_args = 1.., conflicts_with = "all")]
        services: Vec<String>,
        #[arg(long)]
        all: bool,
    },
    /// Show current background service states.
    Status,
    /// Read bounded recent output from a background service.
    Logs {
        #[arg(value_name = "SERVICE")]
        service: String,
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u16).range(1..=500))]
        limit: u16,
        #[arg(long)]
        after: Option<u64>,
    },
    /// Stop all services and end the background workspace.
    Shutdown,

    /// Validate workspace, project, service, and dependency definitions.
    Validate,

    /// Show the dependency-first startup plan without executing commands.
    Plan {
        /// Services to select; defaults to autostart services.
        #[arg(value_name = "SERVICE")]
        targets: Vec<String>,
    },

    /// Run dependency-free services in the foreground.
    Run {
        /// Services to run.
        #[arg(value_name = "SERVICE", required_unless_present = "all", num_args = 1.., conflicts_with = "all")]
        services: Vec<String>,
        /// Run all autostart services.
        #[arg(long)]
        all: bool,
    },
}
