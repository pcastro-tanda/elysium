//! Developer-facing project automation, invoked via `cargo xtask <command>`.

mod bench;
mod conformance;
mod conformance_rule;
mod docs_rules;
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
    /// Compare one or more rules' offenses against real RuboCop's on a corpus app.
    Conformance(conformance_rule::ConformanceArgs),
    /// Regenerate `docs/rules` from the registered rules' metadata.
    DocsRules,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Bench(args) => bench::run(&args),
        Command::ConformanceConfig(args) => conformance::run(&args),
        Command::Conformance(args) => conformance_rule::run(&args),
        Command::DocsRules => docs_rules::run(),
    };
    match result {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

/// The release `elysium` binary, honouring `CARGO_TARGET_DIR` (relative
/// paths resolve against the workspace, as cargo does) so isolated build
/// directories see their own build.
fn release_binary(workspace: &std::path::Path) -> std::path::PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map_or_else(|| workspace.join("target"), |dir| workspace.join(dir));
    target.join("release/elysium")
}
