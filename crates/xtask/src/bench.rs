//! `cargo xtask bench`: builds the release CLI, runs it end-to-end against a
//! Ruby corpus a handful of times, and compares the resulting medians
//! against `benchmarks/results.json`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{anyhow, bail, Context as _, Result};
use clap::Args;
use serde::{Deserialize, Serialize};

use crate::rubocop;
use crate::stats;
use crate::time::iso8601_now;

/// Corpus checked out alongside this repository for end-to-end benchmarking.
const DEFAULT_CORPUS: &str = "/Users/paulo/Work/lab/corpus/gitlab";

/// Path to the checked-in benchmark results, relative to the workspace root.
const RESULTS_PATH: &str = "benchmarks/results.json";

/// Regression threshold enforced by `--check`: fresh medians may not exceed
/// the recorded median by more than this fraction.
const REGRESSION_THRESHOLD: f64 = 0.05;

/// `cargo xtask bench` arguments.
#[derive(Debug, Args)]
pub(crate) struct BenchArgs {
    /// Overwrite `benchmarks/results.json` with the freshly measured medians.
    #[arg(long)]
    record: bool,

    /// Compare fresh medians against `benchmarks/results.json` and fail on
    /// a regression greater than 5%.
    #[arg(long)]
    check: bool,

    /// Ruby corpus to lint. Defaults to the shared GitLab checkout.
    #[arg(long, default_value = DEFAULT_CORPUS)]
    corpus: PathBuf,

    /// Number of end-to-end runs to take the median of.
    #[arg(long, default_value_t = 5)]
    runs: usize,

    /// Also benchmark RuboCop (`--only Lint/Syntax`) over the same corpus
    /// and report the speedup. Off by default: RuboCop takes minutes.
    #[arg(long)]
    rubocop: bool,
}

/// One recorded benchmark entry in `benchmarks/results.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchResult {
    /// Median value across `runs` samples. Always populated, even for
    /// `unit: "count"` entries such as `e2e/files`.
    median_ms: f64,
    /// Number of samples the median was computed from.
    runs: usize,
    /// ISO 8601 UTC timestamp of when this entry was recorded.
    recorded_at: String,
    /// Host description the benchmark was recorded on.
    host: String,
    /// `"ms"` for durations, `"count"` for plain counters.
    #[serde(default = "default_unit")]
    unit: String,
}

fn default_unit() -> String {
    "ms".to_string()
}

/// One freshly measured benchmark: a median value plus its unit.
type FreshResults = BTreeMap<String, (f64, &'static str)>;

/// Runs the `bench` subcommand.
pub(crate) fn run(args: &BenchArgs) -> Result<ExitCode> {
    if args.runs == 0 {
        bail!("--runs must be at least 1");
    }

    let workspace_root = workspace_root();
    build_release_cli(&workspace_root)?;

    let binary = workspace_root.join("target/release/elysium");
    let corpus = args
        .corpus
        .canonicalize()
        .with_context(|| format!("corpus path {} does not exist", args.corpus.display()))?;

    let mut fresh = run_benchmarks(&binary, &corpus, args.runs)?;
    let mut rubocop_version = None;
    if args.rubocop {
        if let Some(bench) = rubocop::run(&corpus, args.runs)? {
            fresh.insert("rubocop/total".to_string(), (bench.total_ms, "ms"));
            fresh.insert("rubocop/files".to_string(), (bench.files, "count"));
            rubocop_version = Some(bench.version);
        }
    }
    let results_path = workspace_root.join(RESULTS_PATH);
    let previous = load_results(&results_path)?;

    if args.check {
        let recorded = previous.ok_or_else(|| {
            anyhow!(
                "no recorded benchmarks at {}; run `cargo xtask bench --record` first",
                results_path.display()
            )
        })?;
        print_table(&fresh, Some(&recorded));
        if has_regression(&fresh, &recorded) {
            eprintln!(
                "benchmark regression exceeds {:.0}% threshold",
                REGRESSION_THRESHOLD * 100.0
            );
            return Ok(ExitCode::FAILURE);
        }
        return Ok(ExitCode::SUCCESS);
    }

    print_table(&fresh, previous.as_ref());

    if args.record {
        let recorded_at = iso8601_now();
        let host = host_string();
        let new_results: BTreeMap<String, BenchResult> = fresh
            .iter()
            .map(|(name, &(median_ms, unit))| {
                // RuboCop entries carry the tool's version instead of the
                // machine host, so `--check` runs can see which release a
                // recorded RuboCop benchmark came from.
                let entry_host = if name.starts_with("rubocop/") {
                    rubocop_version.clone().unwrap_or_else(|| host.clone())
                } else {
                    host.clone()
                };
                (
                    name.clone(),
                    BenchResult {
                        median_ms,
                        runs: args.runs,
                        recorded_at: recorded_at.clone(),
                        host: entry_host,
                        unit: unit.to_string(),
                    },
                )
            })
            .collect();
        write_results(&results_path, &new_results)?;
    }

    Ok(ExitCode::SUCCESS)
}

/// Locates the workspace root from this crate's manifest directory
/// (`<root>/crates/xtask`), independent of the caller's working directory.
pub(crate) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask crate must live at <workspace-root>/crates/xtask")
        .to_path_buf()
}

/// Runs `cargo build --release -p cli`, inheriting stdio.
pub(crate) fn build_release_cli(workspace_root: &Path) -> Result<()> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(cargo)
        .args(["build", "--release", "-p", "cli"])
        .current_dir(workspace_root)
        .status()
        .context("failed to spawn `cargo build --release -p cli`")?;
    if !status.success() {
        bail!("`cargo build --release -p cli` failed with {status}");
    }
    Ok(())
}

/// Runs the release `elysium` binary against `corpus` `runs` times and
/// returns the median of each tracked metric.
fn run_benchmarks(binary: &Path, corpus: &Path, runs: usize) -> Result<FreshResults> {
    let mut files = Vec::with_capacity(runs);
    let mut discover = Vec::with_capacity(runs);
    let mut lint = Vec::with_capacity(runs);
    let mut total = Vec::with_capacity(runs);

    for run_index in 0..runs {
        let sample = run_once(binary, corpus)
            .with_context(|| format!("benchmark run {} of {runs}", run_index + 1))?;
        files.push(sample.files);
        discover.push(sample.discover_ms);
        lint.push(sample.lint_ms);
        total.push(sample.total_ms);
    }

    let mut results = BTreeMap::new();
    results.insert("e2e/discover".to_string(), (median(discover), "ms"));
    results.insert("e2e/lint".to_string(), (median(lint), "ms"));
    results.insert("e2e/total".to_string(), (median(total), "ms"));
    results.insert("e2e/files".to_string(), (median(files), "count"));
    Ok(results)
}

/// Runs `elysium check --stats <corpus>` once with `cwd` set to `corpus`
/// and parses the resulting stats line from stderr.
fn run_once(binary: &Path, corpus: &Path) -> Result<stats::RunStats> {
    let output = Command::new(binary)
        .arg("check")
        .arg("--stats")
        .arg(corpus)
        .current_dir(corpus)
        .output()
        .with_context(|| format!("failed to execute {}", binary.display()))?;

    match output.status.code() {
        Some(0 | 1) => {}
        other => bail!("elysium exited with unexpected status {other:?}"),
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    stats::parse_stderr(&stderr)
}

/// Median of a slice of samples; averages the two middle samples for an
/// even-sized input.
pub(crate) fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let len = values.len();
    if len % 2 == 1 {
        values[len / 2]
    } else {
        f64::midpoint(values[len / 2 - 1], values[len / 2])
    }
}

/// Loads `benchmarks/results.json`, returning `None` if it does not exist.
fn load_results(path: &Path) -> Result<Option<BTreeMap<String, BenchResult>>> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let results = serde_json::from_str(&contents)
                .with_context(|| format!("parsing {}", path.display()))?;
            Ok(Some(results))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
    }
}

/// Writes `results` to `path` as pretty, key-sorted JSON.
fn write_results(path: &Path, results: &BTreeMap<String, BenchResult>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(results).context("serializing benchmark results")?;
    fs::write(path, format!("{json}\n")).with_context(|| format!("writing {}", path.display()))
}

/// Returns true if any duration (`unit: "ms"`) benchmark regressed by more
/// than [`REGRESSION_THRESHOLD`] relative to its recorded median.
fn has_regression(fresh: &FreshResults, recorded: &BTreeMap<String, BenchResult>) -> bool {
    fresh.iter().any(|(name, &(fresh_ms, unit))| {
        if unit != "ms" {
            return false;
        }
        let Some(previous) = recorded.get(name) else { return false };
        if previous.median_ms <= 0.0 {
            return false;
        }
        (fresh_ms - previous.median_ms) / previous.median_ms > REGRESSION_THRESHOLD
    })
}

/// Prints a `name  recorded  fresh  delta%` table to stdout.
fn print_table(fresh: &FreshResults, recorded: Option<&BTreeMap<String, BenchResult>>) {
    println!("{:<16}{:>14}{:>14}{:>10}", "name", "recorded", "fresh", "delta%");
    for (name, &(fresh_value, unit)) in fresh {
        let previous = recorded.and_then(|r| r.get(name));
        let recorded_str =
            previous.map_or_else(|| "-".to_string(), |p| format_value(p.median_ms, unit));
        let fresh_str = format_value(fresh_value, unit);
        let delta_str = previous.map_or_else(
            || "-".to_string(),
            |p| {
                if p.median_ms > 0.0 {
                    format!("{:+.1}%", (fresh_value - p.median_ms) / p.median_ms * 100.0)
                } else {
                    "n/a".to_string()
                }
            },
        );
        println!("{name:<16}{recorded_str:>14}{fresh_str:>14}{delta_str:>10}");
    }

    if let (Some(&(rubocop_total, _)), Some(&(e2e_total, _))) =
        (fresh.get("rubocop/total"), fresh.get("e2e/total"))
    {
        if e2e_total > 0.0 {
            println!("speedup (rubocop/total \u{f7} e2e/total): {:.1}x", rubocop_total / e2e_total);
        }
    }

    if let (Some(&(rubocop_files, _)), Some(&(e2e_files, _))) =
        (fresh.get("rubocop/files"), fresh.get("e2e/files"))
    {
        if (rubocop_files - e2e_files).abs() > f64::EPSILON {
            println!(
                "file count differs: e2e/files={e2e_files:.0} rubocop/files={rubocop_files:.0} (diff {:+.0})",
                rubocop_files - e2e_files
            );
        }
    }
}

/// Formats a raw metric value for table display.
fn format_value(value: f64, unit: &str) -> String {
    if unit == "count" {
        format!("{value:.0}")
    } else {
        format!("{value:.1}ms")
    }
}

/// Best-effort host description: `uname -m` plus, on macOS, the CPU brand
/// string. Falls back to `"unknown"` if neither command succeeds.
fn host_string() -> String {
    let arch = command_output("uname", &["-m"]);
    let cpu = command_output("sysctl", &["-n", "machdep.cpu.brand_string"]);
    let combined = [arch, cpu].into_iter().flatten().collect::<Vec<_>>().join(" ");
    if combined.is_empty() {
        "unknown".to_string()
    } else {
        combined
    }
}

/// Runs `program args...` and returns its trimmed stdout, or `None` on any
/// failure.
fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
