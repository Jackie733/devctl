use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about = "Development workspace manager")]
pub struct Cli {
    /// Path to the workspace configuration.
    #[arg(
        long,
        global = true,
        default_value = "devctl.toml",
        value_name = "PATH"
    )]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Validate workspace, project, service, and dependency definitions.
    Validate,

    /// Show the dependency-first startup plan without executing commands.
    Plan {
        /// Services to select; defaults to autostart services.
        #[arg(value_name = "SERVICE")]
        targets: Vec<String>,
    },

    /// Run one dependency-free service in the foreground.
    Run {
        /// Service to run.
        #[arg(value_name = "SERVICE")]
        service: String,
    },
}
