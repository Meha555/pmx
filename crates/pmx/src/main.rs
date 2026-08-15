mod analyze;
mod capability;
mod cli;
mod config;
mod platform;
mod registry;
mod report;
mod store;
mod util;
mod wrap;

#[cfg(feature = "dynamic")]
mod loader;

use clap::Parser;

use crate::cli::{Cli, Command};
use crate::util::output::print_json_value;

fn main() {
    let cli = Cli::parse();
    let json = cli.json();
    let registry = match registry::available(cli.module_dir().map(|path| path.as_path())) {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    };
    let result = match cli.command {
        Command::Modules(args) => cli::commands::modules(args, &registry),
        Command::Check(args) => cli::commands::check(args, &registry),
        Command::Wrap(args) => cli::commands::wrap(args, &registry),
        Command::Analyze(args) => cli::commands::analyze(args, &registry),
        Command::Report(args) => cli::commands::report(args, &registry),
    };

    match result {
        Ok(value) => {
            if json {
                print_json_value(&value);
            }
        }
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    }
}
