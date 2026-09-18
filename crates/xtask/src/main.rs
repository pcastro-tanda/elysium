//! Developer-facing project automation, invoked via `cargo xtask <command>`.

mod bench;
mod conformance;
mod rubocop;
mod stats;
mod time;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Top-level `cargo xtask` command line.
#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Project automation for elysium")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Available `xtask` subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Build the release CLI and run end-to-end corpus benchmarks.
    Bench(bench::BenchArgs),
    /// Compare `elysium config --format show-cops` against a ground-truth
    /// `rubocop --show-cops` capture, cop by cop.
    ConformanceConfig(conformance::ConformanceConfigArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Bench(args) => bench::run(&args),
        Command::ConformanceConfig(args) => conformance::run(&args),
    };
    match result {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
