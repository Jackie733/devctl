use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Development workspace manager",
    after_help = "Run without a subcommand to open the terminal interface."
)]
pub struct Cli {
    /// Config path; otherwise search ./devctl.toml, ~/devctl.toml, ~/.config/devctl/devctl.toml.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check declared Node environments without installing tools or changing bindings.
    Doctor {
        #[arg(value_name = "SERVICE")]
        services: Vec<String>,
        #[arg(long, value_name = "ABSOLUTE_PATH")]
        node: Option<PathBuf>,
    },
    /// Preview or record bindings to existing Node tools (installation is not available yet).
    Setup {
        #[arg(value_name = "SERVICE")]
        services: Vec<String>,
        #[arg(long, value_name = "ABSOLUTE_PATH")]
        node: Option<PathBuf>,
        #[arg(long)]
        apply: bool,
        #[arg(long, requires = "apply")]
        yes: bool,
    },
    /// Preview project directories and declared scripts without executing or writing.
    Import {
        #[arg(value_name = "DIR")]
        directory: PathBuf,
    },

    /// Check, install, or roll back the devctl program (no workspace config needed).
    Update {
        /// Check the latest stable release without changing the installation.
        #[arg(long, conflicts_with = "rollback")]
        check: bool,
        /// Install or check a specific stable version, for example 1.2.2.
        #[arg(long, value_name = "VERSION", conflicts_with = "rollback")]
        version: Option<String>,
        /// Exchange the installed program with its previous version, without networking.
        #[arg(long)]
        rollback: bool,
    },

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
        /// Acknowledge unknown dependency inventory for this request only.
        #[arg(long)]
        allow_unverified_dependencies: bool,
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
    /// List up to 100 retained log sessions without starting services.
    LogSessions,
    /// Read bounded service output; without a background, read the latest saved session.
    Logs {
        #[arg(value_name = "SERVICE")]
        service: String,
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u16).range(1..=500))]
        limit: u16,
        /// Read records strictly after this sequence.
        #[arg(long, conflicts_with = "before")]
        after: Option<u64>,
        /// Read the nearest records strictly before this sequence.
        #[arg(long, conflicts_with = "after")]
        before: Option<u64>,
        /// Read this saved session (see log-sessions).
        #[arg(long)]
        session: Option<String>,
        /// Case-sensitive literal text search across retained records.
        #[arg(long)]
        contains: Option<String>,
        /// Fix the inclusive search upper bound (reported by the first page).
        #[arg(long)]
        upper: Option<u64>,
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
        /// Acknowledge unknown dependency inventory for this run only.
        #[arg(long)]
        allow_unverified_dependencies: bool,
    },
}
