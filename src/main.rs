mod analyze;
mod capability;
mod cli;
mod config;
mod model;
mod modules;
mod platform;
mod report;
mod store;
mod util;
mod wrap;

use clap::Parser;

use crate::cli::{Cli, Command};
use crate::util::output::print_json_value;

fn main() {
    let cli = Cli::parse();
    let json = cli.json();
    let result = match cli.command {
        Command::Modules(args) => cli::commands::modules(args),
        Command::Check(args) => cli::commands::check(args),
        Command::Wrap(args) => cli::commands::wrap(args),
        Command::Analyze(args) => cli::commands::analyze(args),
        Command::Report(args) => cli::commands::report(args),
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
