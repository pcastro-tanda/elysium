//! `cargo xtask conformance --app DIR --rule Cop/Name`: compares one rule's
//! offenses against real RuboCop's on a corpus application.
//!
//! RuboCop is the ground truth and is slow (minutes on a large app), so its
//! JSON report is cached next to the corpus checkout and only re-run with
//! `--refresh`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context as _, Result};
use clap::Args;
use serde::Deserialize;

use crate::bench::{build_release_cli, workspace_root};

/// Where the per-rule conformance table lives, relative to the workspace root.
const TABLE_PATH: &str = "docs/conformance/rules.md";

/// The rbenv Ruby version under which every `<app>.rubocop.Gemfile` side
/// bundle is installed (see [`side_gemfile`]).
const SIDE_GEMFILE_RUBY_VERSION: &str = "3.4.2";

/// `cargo xtask conformance` arguments.
#[derive(Debug, Args)]
pub(crate) struct ConformanceArgs {
    /// Application checkout to run both linters over.
    #[arg(long, value_name = "PATH")]
    app: PathBuf,

    /// Cop to compare, e.g. `Layout/TrailingWhitespace`.
    #[arg(long, value_name = "COP")]
    rule: String,

    /// Ignore the app's own configuration on both sides.
    #[arg(long)]
    defaults: bool,

    /// Re-run RuboCop even when a cached report exists.
    #[arg(long)]
    refresh: bool,

    /// Print the comparison without updating `docs/conformance/rules.md`.
    #[arg(long)]
    no_write: bool,
}

/// RuboCop's (and elysium's) JSON report, reduced to what is compared.
#[derive(Debug, Deserialize)]
struct Report {
    files: Vec<FileReport>,
    #[serde(default)]
    metadata: Metadata,
}

#[derive(Debug, Default, Deserialize)]
struct Metadata {
    #[serde(default)]
    rubocop_version: String,
}

#[derive(Debug, Deserialize)]
struct FileReport {
    path: String,
    offenses: Vec<Offense>,
}

#[derive(Debug, Deserialize)]
struct Offense {
    message: String,
    location: Location,
}

#[derive(Debug, Deserialize)]
struct Location {
    start_line: u32,
    start_column: u32,
}

/// One offense, keyed the way the comparison identifies it.
type Key = (String, u32, u32);

pub(crate) fn run(args: &ConformanceArgs) -> Result<ExitCode> {
    let app = args.app.canonicalize().with_context(|| format!("{}", args.app.display()))?;
    let workspace = workspace_root();

    let (truth_json, cache_path, from_cache) = truth_report(&app, args)?;
    let truth: Report =
        serde_json::from_str(&truth_json).context("parsing cached rubocop report")?;
    let rubocop_version = if truth.metadata.rubocop_version.is_empty() {
        "unknown".to_string()
    } else {
        truth.metadata.rubocop_version.clone()
    };
    println!(
        "truth: rubocop {rubocop_version} ({}{})",
        cache_path.display(),
        if from_cache { ", cached" } else { ", fresh" }
    );

    build_release_cli(&workspace)?;
    let ours_json = run_elysium(&workspace, &app, &args.rule, args.defaults)?;
    let ours: Report = serde_json::from_str(&ours_json).context("parsing elysium report")?;

    let truth_offenses = index(&truth);
    let our_offenses = index(&ours);

    let mut matched = 0usize;
    let mut message_mismatch = 0usize;
    let mut missing: Vec<&Key> = Vec::new();
    for (key, message) in &truth_offenses {
        match our_offenses.get(key) {
            Some(ours) => {
                matched += 1;
                if ours != message {
                    message_mismatch += 1;
                }
            }
            None => missing.push(key),
        }
    }
    let extra: Vec<&Key> =
        our_offenses.keys().filter(|key| !truth_offenses.contains_key(*key)).collect();

    let agreement = {
        let denominator = truth_offenses.len().max(our_offenses.len());
        if denominator == 0 {
            1.0
        } else {
            #[allow(clippy::cast_precision_loss)]
            {
                matched as f64 / denominator as f64
            }
        }
    };

    println!(
        "rule: {}  app: {}  truth: {}  ours: {}  matched: {matched}  missing: {}  extra: {}  message_mismatch: {message_mismatch}  agreement: {:.2}%",
        args.rule,
        app.file_name().unwrap_or_default().to_string_lossy(),
        truth_offenses.len(),
        our_offenses.len(),
        missing.len(),
        extra.len(),
        agreement * 100.0,
    );
    print_samples(&app, "missing (false negatives)", &missing);
    print_samples(&app, "extra (false positives)", &extra);

    if !args.no_write {
        let row = Row {
            rule: args.rule.clone(),
            app: app.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            defaults: args.defaults,
            rubocop_version,
            truth: truth_offenses.len(),
            ours: our_offenses.len(),
            missing: missing.len(),
            extra: extra.len(),
            message_mismatch,
            agreement,
        };
        let table = workspace.join(TABLE_PATH);
        update_table(&table, &row)?;
        println!("wrote {}", table.display());
    }

    Ok(ExitCode::SUCCESS)
}

/// Offenses keyed by `(relative path, line, column)`, mapped to their message.
fn index(report: &Report) -> BTreeMap<Key, String> {
    let mut out = BTreeMap::new();
    for file in &report.files {
        let path = file.path.trim_start_matches("./").to_string();
        for offense in &file.offenses {
            out.insert(
                (path.clone(), offense.location.start_line, offense.location.start_column),
                offense.message.clone(),
            );
        }
    }
    out
}

fn print_samples(app: &Path, label: &str, keys: &[&Key]) {
    if keys.is_empty() {
        return;
    }
    println!("\n{label}: {} (showing up to 20)", keys.len());
    for key in keys.iter().take(20) {
        let (path, line, column) = key;
        let text = fs::read_to_string(app.join(path))
            .ok()
            .and_then(|source| source.lines().nth(*line as usize - 1).map(str::to_string))
            .unwrap_or_default();
        println!("  {path}:{line}:{column}  {}", text.replace('\t', "\\t"));
    }
}

/// The RuboCop report for this app and rule, from cache or a fresh run.
fn truth_report(app: &Path, args: &ConformanceArgs) -> Result<(String, PathBuf, bool)> {
    let name = app.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let slug = args.rule.replace('/', "__");
    let suffix = if args.defaults { ".defaults" } else { "" };
    let parent = app.parent().unwrap_or_else(|| Path::new("."));
    let cache = parent.join(format!("{name}.rule.{slug}{suffix}.json"));

    if !args.refresh {
        if let Ok(cached) = fs::read_to_string(&cache) {
            if !cached.trim().is_empty() {
                return Ok((cached, cache, true));
            }
        }
    }

    let json = run_rubocop(app, &args.rule, args.defaults)?;
    fs::write(&cache, &json).with_context(|| format!("writing {}", cache.display()))?;
    Ok((json, cache, false))
}

/// True when the app's `Gemfile.lock` pins RuboCop, so it must be run
/// through Bundler to get that version and its plugins.
fn uses_bundler(app: &Path) -> bool {
    fs::read_to_string(app.join("Gemfile.lock"))
        .is_ok_and(|lock| lock.lines().any(|line| line.trim_start().starts_with("rubocop ")))
}

/// A side Gemfile at `<corpus>/<app-name>.rubocop.Gemfile`, next to the app
/// checkout, pinning RuboCop and its plugins independently of the app's own
/// `Gemfile`. Used when the app's own bundle can't be installed (e.g. a
/// `.ruby-version` pin unavailable via rbenv, or a native extension that
/// fails to build) but a scratch bundle for RuboCop alone still can be.
fn side_gemfile(app: &Path) -> Option<PathBuf> {
    let name = app.file_name()?.to_string_lossy().into_owned();
    let parent = app.parent().unwrap_or_else(|| Path::new("."));
    let candidate = parent.join(format!("{name}.rubocop.Gemfile"));
    candidate.is_file().then_some(candidate)
}

/// Runs RuboCop and returns its JSON report.
///
/// Tried in order: a side Gemfile pinning RuboCop for this app (see
/// [`side_gemfile`]), then the app's own bundle when its `Gemfile.lock` pins
/// RuboCop, then the `rubocop` on `PATH`. Each step falls through to the
/// next on failure rather than failing outright, since the corpus checkouts
/// commonly can't install their full bundle on this machine.
fn run_rubocop(app: &Path, rule: &str, defaults: bool) -> Result<String> {
    if let Some(gemfile) = side_gemfile(app) {
        match rubocop_once(app, rule, defaults, true, Some(&gemfile)) {
            Ok(json) => return Ok(json),
            Err(err) => eprintln!(
                "note: `BUNDLE_GEMFILE={} bundle exec rubocop` failed ({err}); falling back",
                gemfile.display()
            ),
        }
    }
    if uses_bundler(app) {
        match rubocop_once(app, rule, defaults, true, None) {
            Ok(json) => return Ok(json),
            Err(err) => eprintln!(
                "note: `bundle exec rubocop` failed ({err}); falling back to the `rubocop` on PATH"
            ),
        }
    }
    rubocop_once(app, rule, defaults, false, None)
}

fn rubocop_once(
    app: &Path,
    rule: &str,
    defaults: bool,
    bundler: bool,
    gemfile: Option<&Path>,
) -> Result<String> {
    let mut command = if bundler {
        let mut command = Command::new("bundle");
        command.arg("exec").arg("rubocop");
        command
    } else {
        Command::new("rubocop")
    };
    command
        .args(["--only", rule, "--format", "json", "--cache", "false"])
        .current_dir(app)
        .env("RUBOCOP_CACHE_ROOT", std::env::temp_dir().join("xtask-rubocop-cache"));
    if let Some(gemfile) = gemfile {
        command.env("BUNDLE_GEMFILE", gemfile);
        // Corpus apps often pin a `.ruby-version` unavailable on this
        // machine (e.g. an exact patch release rbenv never built). The side
        // Gemfile's bundle was installed under this interpreter, so force
        // rbenv to use it regardless of the app's own pin.
        command.env("RBENV_VERSION", SIDE_GEMFILE_RUBY_VERSION);
    }
    if defaults {
        command.arg("--force-default-config");
    }
    eprintln!(
        "running {}{}rubocop --only {rule}{} in {} ...",
        gemfile.map(|g| format!("BUNDLE_GEMFILE={} ", g.display())).unwrap_or_default(),
        if bundler { "bundle exec " } else { "" },
        if defaults { " --force-default-config" } else { "" },
        app.display()
    );
    let output = command.output().context("failed to execute rubocop")?;
    match output.status.code() {
        Some(0 | 1) => {}
        other => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr
                .lines()
                .find(|line| {
                    !line.starts_with("Source locally installed") && !line.starts_with("Ignoring ")
                })
                .unwrap_or("no stderr");
            bail!("rubocop exited with {other:?}: {detail}");
        }
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let start = stdout.find('{').context("rubocop produced no JSON report")?;
    Ok(stdout[start..].to_string())
}

fn run_elysium(workspace: &Path, app: &Path, rule: &str, defaults: bool) -> Result<String> {
    let binary = workspace.join("target/release/elysium");
    let mut command = Command::new(&binary);
    command.args(["check", "--only", rule, "-f", "json"]).current_dir(app);
    if defaults {
        command.arg("--no-config");
    }
    command.arg(".");
    let output = command.output().context("failed to execute elysium")?;
    match output.status.code() {
        Some(0 | 1) => {}
        other => {
            bail!("elysium exited with {other:?}:\n{}", String::from_utf8_lossy(&output.stderr))
        }
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// One row of `docs/conformance/rules.md`.
struct Row {
    rule: String,
    app: String,
    defaults: bool,
    rubocop_version: String,
    truth: usize,
    ours: usize,
    missing: usize,
    extra: usize,
    message_mismatch: usize,
    agreement: f64,
}

impl Row {
    /// `rule | app` identity: a re-run replaces the previous row.
    fn key(&self) -> String {
        format!(
            "| {} | {}{} |",
            self.rule,
            self.app,
            if self.defaults { " (defaults)" } else { "" }
        )
    }

    fn render(&self) -> String {
        format!(
            "{} {} | {} | {} | {} | {} | {} | {:.1}% | {} |",
            self.key(),
            self.rubocop_version,
            self.truth,
            self.ours,
            self.missing,
            self.extra,
            self.message_mismatch,
            self.agreement * 100.0,
            today(),
        )
    }
}

const TABLE_HEADER: &str = "\
# Per-rule conformance

Generated by `cargo xtask conformance --app DIR --rule Cop/Name`. Offenses are
keyed by (path, line, column); `message mismatch` counts matched offenses whose
text differs (RuboCop versions word some messages differently).

| rule | app | rubocop | truth | ours | missing | extra | message mismatch | agreement | date |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
";

fn update_table(path: &Path, row: &Row) -> Result<()> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    let mut rows: Vec<String> = existing
        .lines()
        .filter(|line| {
            line.starts_with("| ") && !line.starts_with("| rule |") && !line.starts_with("| --- |")
        })
        .map(str::to_string)
        .collect();
    let key = row.key();
    rows.retain(|line| !line.starts_with(&key));
    rows.push(row.render());
    rows.sort();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = String::from(TABLE_HEADER);
    for line in rows {
        out.push_str(&line);
        out.push('\n');
    }
    fs::write(path, out).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Today's date as `YYYY-MM-DD` (UTC), without pulling in a date crate.
fn today() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Howard Hinnant's `civil_from_days`.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    (year, m as u32, d as u32)
}
