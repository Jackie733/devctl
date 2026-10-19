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
    /// Add a directory and explicit command, for any language (preview by default).
    Add {
        #[arg(value_name = "DIR")]
        directory: PathBuf,
        #[arg(long, value_name = "ID")]
        name: Option<String>,
        #[arg(long)]
        apply: bool,
        #[arg(long, requires = "apply")]
        yes: bool,
        /// Command and arguments after --; no implicit shell is used.
        #[arg(last = true, required = true, value_name = "COMMAND", num_args = 1..)]
        command: Vec<String>,
    },
    /// Preview project directories and review configuration additions or replacements.
    Import {
        #[arg(value_name = "DIR")]
        directory: PathBuf,
        #[command(flatten)]
        options: ImportOptions,
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
    },
    /// Stop selected services, keeping the workspace available.
    Stop {
        #[arg(value_name = "SERVICE", required_unless_present = "all", num_args = 1.., conflicts_with = "all")]
        services: Vec<String>,
        #[arg(long)]
        all: bool,
    },
    /// Stop and restart selected services in a background workspace.
    Restart {
        #[arg(value_name = "SERVICE", required_unless_present = "all", num_args = 1.., conflicts_with = "all")]
        services: Vec<String>,
        #[arg(long)]
        all: bool,
    },
    /// Reload configuration changes for stopped services in an active background workspace.
    Reload,
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
    },
}

#[derive(Debug, Clone, Default, clap::Args)]
pub struct ImportOptions {
    #[arg(long = "project", value_name = "RELATIVE_PATH")]
    pub projects: Vec<PathBuf>,
    #[arg(long = "script", value_name = "RELATIVE_PATH=SCRIPT")]
    pub scripts: Vec<String>,
    /// Select only the command name when discovery has no package-manager evidence.
    #[arg(long, value_parser = ["npm", "pnpm"])]
    pub package_manager: Option<String>,
    #[arg(long, value_name = "PROJECT_ID")]
    pub replace: Vec<String>,
    #[arg(long)]
    pub apply: bool,
    #[arg(long, requires = "apply")]
    pub yes: bool,
}
