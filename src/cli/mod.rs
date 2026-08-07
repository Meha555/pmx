mod analyze;
mod check;
mod modules;
mod report;
mod shared;
mod wrap;

pub mod commands {
    pub use super::analyze::analyze;
    pub use super::check::check;
    pub use super::modules::modules;
    pub use super::report::report;
    pub use super::wrap::wrap;
}

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "pmx", version, about = "Process monitor and diagnostics CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn json(&self) -> bool {
        match &self.command {
            Command::Modules(args) => args.json,
            Command::Check(args) => args.json,
            Command::Wrap(args) => args.json,
            Command::Analyze(args) => args.json,
            Command::Report(args) => args.json,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// List built-in PMX modules and their required capabilities.
    Modules(ModulesArgs),
    /// Check whether configured PMX capabilities are supported in this environment.
    Check(CheckArgs),
    /// Wrap a pressure-test command and collect run evidence.
    Wrap(WrapArgs),
    /// Analyze an existing run and persist findings.
    Analyze(AnalyzeArgs),
    /// Render reports for an analyzed run.
    Report(ReportArgs),
}

#[derive(Args, Debug)]
pub struct ModulesArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct CheckArgs {
    #[arg(short, long, value_name = "FILE")]
    pub config: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct WrapArgs {
    #[arg(short, long, value_name = "FILE")]
    pub config: PathBuf,
    /// Monitor this root PID. Can be repeated. If omitted, PMX monitors the wrapped command process.
    #[arg(long, value_name = "PID")]
    pub pid: Vec<i32>,
    /// Follow child processes of the selected root PIDs, similar to strace -f.
    #[arg(short = 'f', long)]
    pub follow_children: bool,
    #[arg(long)]
    pub json: bool,
    #[arg(last = true, required = true)]
    pub command: Vec<String>,
}

#[derive(Args, Debug)]
pub struct AnalyzeArgs {
    #[arg(long, value_name = "SESSION_ID")]
    pub session: String,
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ReportArgs {
    #[arg(long, value_name = "SESSION_ID")]
    pub session: String,
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}
