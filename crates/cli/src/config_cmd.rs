//! `elysium config`: resolve and print the effective RuboCop configuration.

use std::io::Write as _;
use std::process::ExitCode;

use crate::args::{ConfigArgs, ConfigFormat};
use crate::config_load::load_config;

/// Runs the `config` subcommand, returning the process exit code.
pub fn run(args: &ConfigArgs) -> ExitCode {
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(err) => {
            eprintln!("error: cannot determine working directory: {err}");
            return ExitCode::from(2);
        }
    };

    let resolved = match load_config(args.config.as_deref(), args.no_config, &cwd) {
        Ok(resolved) => resolved,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };

    for warning in resolved.warnings() {
        eprintln!("warning: {warning}");
    }

    let only: Option<Vec<&str>> = if args.only.is_empty() {
        None
    } else {
        Some(args.only.iter().map(String::as_str).collect())
    };

    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let result = match args.format {
        ConfigFormat::ShowCops | ConfigFormat::Yaml => {
            resolved.render_show_cops(&mut out, only.as_deref())
        }
    };

    if let Err(err) = result.and_then(|()| out.flush()) {
        eprintln!("error: {err}");
        return ExitCode::from(2);
    }

    ExitCode::SUCCESS
}
