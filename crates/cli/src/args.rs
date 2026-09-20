use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// A fast, RuboCop-compatible Ruby linter.
#[derive(Debug, Parser)]
#[command(name = "elysium", version, about, propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Lint files and report offenses.
    Check(CheckArgs),
    /// Show the resolved configuration.
    Config(ConfigArgs),
}

#[derive(Debug, Args)]
pub struct CheckArgs {
    /// Files, directories, or globs to check. Defaults to the current directory.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Output format.
    #[arg(long, short = 'f', value_enum, default_value_t = Format::Human)]
    pub format: Format,

    /// Number of worker threads. Defaults to the number of logical CPUs.
    #[arg(long, short = 'j', value_name = "N")]
    pub jobs: Option<usize>,

    /// Do not consult .gitignore files when discovering targets.
    #[arg(long)]
    pub no_gitignore: bool,

    /// Print timing and node statistics to stderr.
    #[arg(long)]
    pub stats: bool,

    /// Configuration file to load instead of searching for one.
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Ignore any `.rubocop.yml` and use only the bundled defaults.
    #[arg(long)]
    pub no_config: bool,
}

#[derive(Debug, Args)]
pub struct ConfigArgs {
    /// Output format.
    #[arg(long, short = 'f', value_enum, default_value_t = ConfigFormat::ShowCops)]
    pub format: ConfigFormat,

    /// Restrict output to these cops (comma-separated, globs allowed).
    #[arg(long, value_name = "COP,...", value_delimiter = ',')]
    pub only: Vec<String>,

    /// Configuration file to load instead of searching for one.
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Ignore any `.rubocop.yml` and use only the bundled defaults.
    #[arg(long)]
    pub no_config: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ConfigFormat {
    /// RuboCop's `--show-cops` listing.
    ShowCops,
    /// Same listing, in YAML form.
    Yaml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// `path:line:col: S: Cop/Name: message` lines plus a summary.
    Human,
    /// RuboCop's JSON formatter schema.
    Json,
}
