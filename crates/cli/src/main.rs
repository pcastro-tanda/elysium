//! `elysium` command-line entry point.
#![allow(unreachable_pub, missing_docs)]

use std::process::ExitCode;

use clap::Parser;

mod args;
mod check;
mod config_cmd;
mod config_load;
mod discover;
mod fix;
mod output;

fn main() -> ExitCode {
    let cli = args::Cli::parse();
    match cli.command {
        args::Command::Check(args) => match check::run(&args) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("error: {err:#}");
                ExitCode::from(2)
            }
        },
        args::Command::Fix(args) => match fix::run(&args) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("error: {err:#}");
                ExitCode::from(2)
            }
        },
        args::Command::Config(args) => config_cmd::run(&args),
    }
}
