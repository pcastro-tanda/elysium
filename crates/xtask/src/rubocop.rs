//! Runs RuboCop's `Lint/Syntax` cop over the same corpus `elysium check`
//! benchmarks, for an apples-to-apples end-to-end speed comparison.
//!
//! RuboCop is invoked with a temporary config that bypasses the corpus's
//! own `.rubocop.yml` (and any gem inheritance it pulls in), restricting
//! the run to `Lint/Syntax` under the same `parser_prism` engine elysium
//! targets. This mirrors exactly what `elysium check` does today, so the
//! two tools' wall-clock times and inspected-file counts are comparable.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{bail, Context as _, Result};
use serde::Deserialize;

use crate::bench::median;

/// Temporary RuboCop config restricting to `Lint/Syntax` with the same
/// parser engine elysium targets, and the default `Include`/`Exclude` (so
/// the corpus's own config, with its gem inheritance, is bypassed).
const TMP_CONFIG_CONTENTS: &str = "\
AllCops:
  ParserEngine: parser_prism
  TargetRubyVersion: 3.4
  NewCops: disable
  SuggestExtensions: false
";

/// One freshly measured RuboCop benchmark: median total wall-clock time
/// across `runs` invocations (milliseconds), median inspected file count,
/// and the `rubocop --version` string.
pub(crate) struct RubocopBench {
    pub(crate) total_ms: f64,
    pub(crate) files: f64,
    pub(crate) version: String,
}

/// The subset of RuboCop's `--format json` report this benchmark needs.
#[derive(Debug, Deserialize)]
struct RubocopReport {
    summary: RubocopSummary,
}

/// The subset of a [`RubocopReport`]'s `summary` object this benchmark needs.
#[derive(Debug, Deserialize)]
struct RubocopSummary {
    inspected_file_count: f64,
}

/// Returns `rubocop`'s first `--version` line, or `None` if `rubocop` is
/// not on `PATH` (or fails to run).
pub(crate) fn version() -> Option<String> {
    let output = Command::new("rubocop").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()?
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
}

/// Runs `rubocop --only Lint/Syntax` over `corpus` `runs` times and returns
/// the median total time and inspected file count, plus the resolved
/// `rubocop` version.
///
/// Returns `Ok(None)` (after printing a message) if `rubocop` is not on
/// `PATH`, rather than failing the whole benchmark run.
pub(crate) fn run(corpus: &Path, runs: usize) -> Result<Option<RubocopBench>> {
    let Some(version) = version() else {
        eprintln!("rubocop not found on PATH; skipping rubocop benchmarks");
        return Ok(None);
    };

    let config_path = write_tmp_config()?;
    let rbenv_version = rbenv_version_override();
    let result = run_samples(corpus, &config_path, runs, rbenv_version.as_deref());
    let _ = fs::remove_file(&config_path);
    let (total_samples, file_samples) = result?;

    Ok(Some(RubocopBench { total_ms: median(total_samples), files: median(file_samples), version }))
}

/// Runs `runs` timed RuboCop invocations, returning parallel vectors of
/// per-run total milliseconds and inspected file counts.
fn run_samples(
    corpus: &Path,
    config_path: &Path,
    runs: usize,
    rbenv_version: Option<&str>,
) -> Result<(Vec<f64>, Vec<f64>)> {
    let mut totals = Vec::with_capacity(runs);
    let mut files = Vec::with_capacity(runs);
    for run_index in 0..runs {
        let (total_ms, file_count) = run_once(corpus, config_path, rbenv_version)
            .with_context(|| format!("rubocop benchmark run {} of {runs}", run_index + 1))?;
        totals.push(total_ms);
        files.push(file_count);
    }
    Ok((totals, files))
}

/// Writes the temporary RuboCop config to a process-unique path under the
/// system temp directory and returns its path.
fn write_tmp_config() -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!("xtask-rubocop-{}.yml", std::process::id()));
    fs::write(&path, TMP_CONFIG_CONTENTS)
        .with_context(|| format!("writing temp rubocop config {}", path.display()))?;
    Ok(path)
}

/// Returns the Ruby version `rbenv` currently resolves outside the corpus,
/// or `None` if `rbenv` is not in use. The corpus may pin its own
/// `.ruby-version` (e.g. for a Ruby that is not installed); running with
/// `RBENV_VERSION` set to this value keeps `rubocop` on the version this
/// benchmark already resolved on `PATH`, matching `elysium check`'s
/// treatment of the corpus as plain input rather than a Ruby project.
fn rbenv_version_override() -> Option<String> {
    let output = Command::new("rbenv").arg("version-name").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

/// Runs `rubocop` once against `corpus`, timing wall-clock and parsing the
/// inspected file count from its `--format json` report.
fn run_once(corpus: &Path, config_path: &Path, rbenv_version: Option<&str>) -> Result<(f64, f64)> {
    let start = Instant::now();
    let mut command = Command::new("rubocop");
    command
        .args(["--only", "Lint/Syntax", "--format", "json", "--cache", "false", "--parallel", "-c"])
        .arg(config_path)
        .current_dir(corpus)
        // Before `-c` is applied, RuboCop unconditionally resolves a cache
        // root through `ConfigStore#for_pwd`, which walks up from `cwd`
        // for a `.rubocop.yml` and evaluates its `require:`/`inherit_gem:`
        // entries -- even though `--cache false` means the result is never
        // used. On a corpus with its own config (and gems this benchmark
        // does not have installed, e.g. GitLab's `rubocop-rspec`), that
        // crashes the process before it ever reaches our `-c` override.
        // `CacheConfig.root_dir` short-circuits on this env var before
        // performing that lookup, so set it to skip the lookup entirely.
        .env("RUBOCOP_CACHE_ROOT", std::env::temp_dir().join("xtask-rubocop-cache"));
    if let Some(version) = rbenv_version {
        command.env("RBENV_VERSION", version);
    }
    let output = command.output().context("failed to execute `rubocop`")?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

    match output.status.code() {
        Some(0 | 1) => {}
        other => bail!("rubocop exited with unexpected status {other:?}"),
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // RuboCop sometimes prints a plain-text warning to stdout ahead of the
    // JSON report (e.g. "-P/--parallel is being ignored because it is not
    // compatible with --cache false."); skip to the report's opening brace
    // rather than failing on it.
    let json_start = stdout
        .find('{')
        .with_context(|| format!("no JSON object found in rubocop stdout:\n{stdout}"))?;
    let report: RubocopReport = serde_json::from_str(&stdout[json_start..])
        .context("parsing rubocop --format json output")?;
    Ok((elapsed_ms, report.summary.inspected_file_count))
}
