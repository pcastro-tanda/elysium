//! `cargo xtask conformance --app DIR --rule Cop/Name`: compares one or more
//! rules' offenses against real RuboCop's on a corpus application.
//!
//! `--rule` is repeatable and comma-separated (`--rule A,B --rule C`), and
//! `--rules-file FILE` reads one cop per line (`#` starts a comment) for
//! waves of dozens of cops. Every rule in a run is linted in a single
//! RuboCop invocation (`--only A,B,C`) and a single elysium invocation; the
//! results are split per cop, so a wave costs one pass over the app instead
//! of one per cop. Reporting stays per rule.
//!
//! RuboCop is the ground truth and is slow (minutes on a large app), so its
//! JSON report is cached per rule next to the corpus checkout
//! (`<app>.rule.<Dept>__<Cop>.json`) and only re-run for rules without a
//! cache file, or for every rule with `--refresh`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context as _, Result};
use clap::Args;
use serde::Deserialize;
use serde_json::value::RawValue;

use crate::bench::{build_release_cli, workspace_root};

/// Where the per-rule conformance table lives, relative to the workspace root.
const TABLE_PATH: &str = "docs/conformance/rules.md";

/// The rbenv Ruby version under which every `<app>.rubocop.Gemfile` side
/// bundle is installed (see [`side_gemfile`]).
const SIDE_GEMFILE_RUBY_VERSION: &str = "3.4.2";

/// `cargo xtask conformance` arguments.
// Flags are independent CLI switches, not a state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args)]
pub(crate) struct ConformanceArgs {
    /// Application checkout to run both linters over.
    #[arg(long, value_name = "PATH")]
    app: PathBuf,

    /// Cops to compare, e.g. `Layout/TrailingWhitespace`. Repeatable and
    /// comma-separated: `--rule A,B --rule C`.
    #[arg(long, value_name = "COP,...", value_delimiter = ',')]
    rule: Vec<String>,

    /// File listing one cop per line (`#` starts a comment), added to
    /// `--rule`.
    #[arg(long, value_name = "PATH", required_unless_present = "rule")]
    rules_file: Option<PathBuf>,

    /// Ignore the app's own configuration on both sides.
    #[arg(long)]
    defaults: bool,

    /// Re-run RuboCop even when a cached report exists.
    #[arg(long)]
    refresh: bool,

    /// Print the comparison without updating `docs/conformance/rules.md`.
    #[arg(long)]
    no_write: bool,

    /// Only generate/refresh the RuboCop truth caches for the requested
    /// rules, then exit without building or running elysium. Rules elysium
    /// does not implement yet are fine here since elysium never runs.
    #[arg(long)]
    truth_only: bool,
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
    cop_name: String,
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
    let rules = resolve_rules(args)?;

    let truths = truth_reports(&app, args, &rules)?;
    for truth in &truths {
        let report: Report = serde_json::from_str(&truth.json)
            .with_context(|| format!("parsing {}", truth.cache.display()))?;
        let rubocop_version = if report.metadata.rubocop_version.is_empty() {
            "unknown".to_string()
        } else {
            report.metadata.rubocop_version.clone()
        };
        println!(
            "truth: rubocop {rubocop_version} ({}{})",
            truth.cache.display(),
            if truth.from_cache { ", cached" } else { ", fresh" }
        );
    }

    if args.truth_only {
        // Only the RuboCop truth caches were requested: elysium never runs,
        // so rules it doesn't implement yet can't error here.
        return Ok(ExitCode::SUCCESS);
    }

    build_release_cli(&workspace)?;
    let ours_json = run_elysium(&workspace, &app, &rules, args.defaults)?;
    let ours: Report = serde_json::from_str(&ours_json).context("parsing elysium report")?;
    // RuboCop's (and elysium's) `--only` output still emits `Lint/Syntax`
    // offenses alongside the requested cops, so both sides are filtered
    // down to the rule under test before comparing. The syntax count is a
    // property of the app, not of the rule, hence shared by every row.
    let syntax_ours = count_cop(&ours, "Lint/Syntax");

    let mut rows = Vec::with_capacity(truths.len());
    let mut diverged = false;
    for truth in &truths {
        let report: Report = serde_json::from_str(&truth.json)
            .with_context(|| format!("parsing {}", truth.cache.display()))?;
        let rubocop_version = if report.metadata.rubocop_version.is_empty() {
            "unknown".to_string()
        } else {
            report.metadata.rubocop_version.clone()
        };
        let row =
            compare(&app, &truth.rule, &report, &ours, syntax_ours, args.defaults, rubocop_version);
        diverged |= row.missing > 0 || row.extra > 0;
        rows.push(row);
    }

    if !args.no_write {
        let table = workspace.join(TABLE_PATH);
        update_table(&table, &rows)?;
        println!("wrote {}", table.display());
    }

    // A single-rule run always succeeds (its numbers are the report); a
    // wave is a gate, so any rule that still diverges fails the run.
    Ok(if rules.len() > 1 && diverged { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

/// The cops to compare: `--rule` values (repeatable, comma-separated) plus
/// `--rules-file` lines, deduplicated, in the order first named.
fn resolve_rules(args: &ConformanceArgs) -> Result<Vec<String>> {
    let mut rules: Vec<String> = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push = |rule: &str| {
        let rule = rule.trim();
        if !rule.is_empty() && seen.insert(rule.to_string()) {
            rules.push(rule.to_string());
        }
    };
    for rule in &args.rule {
        push(rule);
    }
    if let Some(path) = &args.rules_file {
        let text =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        for line in text.lines() {
            push(line.split('#').next().unwrap_or(""));
        }
    }
    if rules.is_empty() {
        bail!("no cops selected: pass --rule Cop/Name or a non-empty --rules-file");
    }
    Ok(rules)
}

/// Compares one rule's truth report against the shared elysium report,
/// printing the `rule:` summary line and offense samples.
fn compare(
    app: &Path,
    rule: &str,
    truth: &Report,
    ours: &Report,
    syntax_ours: usize,
    defaults: bool,
    rubocop_version: String,
) -> Row {
    let syntax_truth = count_cop(truth, "Lint/Syntax");
    let truth_offenses = index(truth, rule);
    let our_offenses = index(ours, rule);

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
        "rule: {rule}  app: {}  truth: {}  ours: {}  matched: {matched}  missing: {}  extra: {}  message_mismatch: {message_mismatch}  agreement: {:.2}%  syntax_truth: {syntax_truth}  syntax_ours: {syntax_ours}",
        app.file_name().unwrap_or_default().to_string_lossy(),
        truth_offenses.len(),
        our_offenses.len(),
        missing.len(),
        extra.len(),
        agreement * 100.0,
    );
    print_samples(app, "missing (false negatives)", &missing);
    print_samples(app, "extra (false positives)", &extra);

    Row {
        rule: rule.to_string(),
        app: app.file_name().unwrap_or_default().to_string_lossy().into_owned(),
        defaults,
        rubocop_version,
        truth: truth_offenses.len(),
        ours: our_offenses.len(),
        missing: missing.len(),
        extra: extra.len(),
        message_mismatch,
        agreement,
    }
}

/// Offenses keyed by `(relative path, line, column)`, mapped to their
/// message, restricted to those whose `cop_name` is `rule`. RuboCop's (and
/// elysium's) `--only Cop/Name` output still carries other cops' offenses
/// (notably `Lint/Syntax`) alongside the requested one.
fn index(report: &Report, rule: &str) -> BTreeMap<Key, String> {
    let mut out = BTreeMap::new();
    for file in &report.files {
        let path = file.path.trim_start_matches("./").to_string();
        for offense in &file.offenses {
            if offense.cop_name != rule {
                continue;
            }
            out.insert(
                (path.clone(), offense.location.start_line, offense.location.start_column),
                offense.message.clone(),
            );
        }
    }
    out
}

/// Count of offenses in `report` whose `cop_name` is `cop`, e.g. the
/// `Lint/Syntax` offenses RuboCop keeps emitting under `--only`.
fn count_cop(report: &Report, cop: &str) -> usize {
    report.files.iter().flat_map(|file| &file.offenses).filter(|o| o.cop_name == cop).count()
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

/// One rule's RuboCop truth report, from cache or a fresh (batched) run.
struct Truth {
    rule: String,
    json: String,
    cache: PathBuf,
    from_cache: bool,
}

/// The RuboCop reports for this app and every requested rule.
///
/// Rules whose cache file is present (and `--refresh` was not passed) are
/// read from disk; everything else is linted by a *single* RuboCop run over
/// the app with `--only A,B,C`, whose report is then split into the same
/// per-rule cache files a one-rule-at-a-time run would have written.
fn truth_reports(app: &Path, args: &ConformanceArgs, rules: &[String]) -> Result<Vec<Truth>> {
    let mut truths: Vec<Truth> = Vec::with_capacity(rules.len());
    let mut stale: Vec<String> = Vec::new();
    for rule in rules {
        let cache = cache_path(app, rule, args.defaults);
        let cached = if args.refresh {
            None
        } else {
            fs::read_to_string(&cache).ok().filter(|text| !text.trim().is_empty())
        };
        let from_cache = cached.is_some();
        if !from_cache {
            stale.push(rule.clone());
        }
        truths.push(Truth {
            rule: rule.clone(),
            json: cached.unwrap_or_default(),
            cache,
            from_cache,
        });
    }

    if !stale.is_empty() {
        let json = run_rubocop(app, &stale, args.defaults)?;
        for (rule, report) in split_by_rule(&json, &stale)? {
            let truth = truths
                .iter_mut()
                .find(|truth| truth.rule == rule)
                .expect("split reports one entry per requested rule");
            fs::write(&truth.cache, &report)
                .with_context(|| format!("writing {}", truth.cache.display()))?;
            truth.json = report;
        }
    }
    Ok(truths)
}

/// Where one rule's cached RuboCop report lives, next to the app checkout.
fn cache_path(app: &Path, rule: &str, defaults: bool) -> PathBuf {
    let name = app.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let slug = rule.replace('/', "__");
    let suffix = if defaults { ".defaults" } else { "" };
    let parent = app.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!("{name}.rule.{slug}{suffix}.json"))
}

/// A batched `--only A,B,C` report, split into one report per rule, byte for
/// byte as a `--only A` run would have printed it: offense objects and the
/// metadata block are copied verbatim from the batch report, only the file
/// list is refiltered and `summary.offense_count` recomputed.
///
/// An offense from a cop that was not requested (`Lint/Syntax`, which
/// RuboCop emits under any `--only`) belongs to every rule's report, since a
/// single-rule run would have reported it too.
fn split_by_rule(json: &str, rules: &[String]) -> Result<Vec<(String, String)>> {
    let report: RawReport = serde_json::from_str(json).context("parsing batched rubocop report")?;
    let requested: BTreeSet<&str> = rules.iter().map(String::as_str).collect();
    let cops: Vec<Vec<String>> = report
        .files
        .iter()
        .map(|file| {
            file.offenses
                .iter()
                .map(|offense| {
                    serde_json::from_str::<CopName>(offense.get())
                        .map(|named| named.cop_name)
                        .context("parsing offense cop_name")
                })
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<Vec<_>>>()?;

    let mut out = Vec::with_capacity(rules.len());
    for rule in rules {
        let mut text = String::with_capacity(json.len() / rules.len().max(1));
        text.push_str("{\"metadata\":");
        text.push_str(report.metadata.get());
        text.push_str(",\"files\":[");
        let mut offense_count = 0usize;
        for (index, file) in report.files.iter().enumerate() {
            if index > 0 {
                text.push(',');
            }
            text.push_str("{\"path\":");
            text.push_str(file.path.get());
            text.push_str(",\"offenses\":[");
            let mut first = true;
            for (offense, cop) in file.offenses.iter().zip(&cops[index]) {
                if cop != rule && requested.contains(cop.as_str()) {
                    continue;
                }
                if !first {
                    text.push(',');
                }
                first = false;
                offense_count += 1;
                text.push_str(offense.get());
            }
            text.push_str("]}");
        }
        write!(
            text,
            "],\"summary\":{{\"offense_count\":{offense_count},\"target_file_count\":{},\"inspected_file_count\":{}}}}}",
            report.summary.target_file_count, report.summary.inspected_file_count,
        )
        .expect("writing to a String cannot fail");
        out.push((rule.clone(), text));
    }
    Ok(out)
}

/// A RuboCop report kept as raw JSON, so splitting it preserves RuboCop's
/// own serialization of the parts that are copied through.
#[derive(Deserialize)]
struct RawReport<'a> {
    #[serde(borrow)]
    metadata: &'a RawValue,
    #[serde(borrow)]
    files: Vec<RawFile<'a>>,
    #[serde(default)]
    summary: RawSummary,
}

#[derive(Deserialize)]
struct RawFile<'a> {
    #[serde(borrow)]
    path: &'a RawValue,
    #[serde(borrow)]
    offenses: Vec<&'a RawValue>,
}

#[derive(Debug, Default, Deserialize)]
struct RawSummary {
    #[serde(default)]
    target_file_count: u64,
    #[serde(default)]
    inspected_file_count: u64,
}

#[derive(Deserialize)]
struct CopName {
    cop_name: String,
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

/// Runs RuboCop once for every rule in `rules` (`--only A,B,C`) and returns
/// its JSON report.
///
/// Tried in order: a side Gemfile pinning RuboCop for this app (see
/// [`side_gemfile`]), then the app's own bundle when its `Gemfile.lock` pins
/// RuboCop, then the `rubocop` on `PATH`. Each step falls through to the
/// next on failure rather than failing outright, since the corpus checkouts
/// commonly can't install their full bundle on this machine.
fn run_rubocop(app: &Path, rules: &[String], defaults: bool) -> Result<String> {
    let only = rules.join(",");
    if let Some(gemfile) = side_gemfile(app) {
        match rubocop_once(app, &only, defaults, true, Some(&gemfile)) {
            Ok(json) => return Ok(json),
            Err(err) => eprintln!(
                "note: `BUNDLE_GEMFILE={} bundle exec rubocop` failed ({err}); falling back",
                gemfile.display()
            ),
        }
    }
    if uses_bundler(app) {
        match rubocop_once(app, &only, defaults, true, None) {
            Ok(json) => return Ok(json),
            Err(err) => eprintln!(
                "note: `bundle exec rubocop` failed ({err}); falling back to the `rubocop` on PATH"
            ),
        }
    }
    rubocop_once(app, &only, defaults, false, None)
}

fn rubocop_once(
    app: &Path,
    only: &str,
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
        .args(["--only", only, "--format", "json", "--cache", "false"])
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
        "running {}{}rubocop --only {only}{} in {} ...",
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

/// Runs elysium under the same `BUNDLE_GEMFILE` the RuboCop side uses when a
/// side Gemfile exists, so both resolve `plugins:`/`require:` gem versions
/// (and thereby the gems' shipped `config/default.yml` layers) from the same
/// lockfile. Without it elysium reads the app's own `Gemfile.lock`, whose
/// plugin versions are typically not installed here, and silently lints
/// with the plugin defaults missing (mastodon: rubocop-rails 2.38.0 locked
/// vs 2.37.0 installed dropped `Lint/NumberConversion`'s `AllowedMethods`).
/// Like RuboCop, elysium is run once for the whole wave (`--only A,B,C`);
/// the report is filtered per rule when comparing.
fn run_elysium(workspace: &Path, app: &Path, rules: &[String], defaults: bool) -> Result<String> {
    let binary = crate::release_binary(workspace);
    let mut command = Command::new(&binary);
    command.args(["check", "--only", &rules.join(","), "-f", "json"]).current_dir(app);
    if let Some(gemfile) = side_gemfile(app) {
        command.env("BUNDLE_GEMFILE", gemfile);
    }
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

Generated by `cargo xtask conformance --app DIR --rule Cop/Name` (`--rule` is
repeatable and comma-separated; `--rules-file FILE` takes one cop per line).
Offenses are keyed by (path, line, column); `message mismatch` counts matched
offenses whose text differs (RuboCop versions word some messages differently).

| rule | app | rubocop | truth | ours | missing | extra | message mismatch | agreement | date |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
";

/// Rewrites the table with `new_rows` replacing any previous row for the
/// same `rule | app` identity.
fn update_table(path: &Path, new_rows: &[Row]) -> Result<()> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    let mut rows: Vec<String> = existing
        .lines()
        .filter(|line| {
            line.starts_with("| ") && !line.starts_with("| rule |") && !line.starts_with("| --- |")
        })
        .map(str::to_string)
        .collect();
    for row in new_rows {
        let key = row.key();
        rows.retain(|line| !line.starts_with(&key));
        rows.push(row.render());
    }
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{resolve_rules, split_by_rule, ConformanceArgs};

    /// An offense as RuboCop serializes it, with its exact key order.
    fn offense(cop: &str, line: u32) -> String {
        format!(
            "{{\"severity\":\"convention\",\"message\":\"{cop} here\",\"cop_name\":\"{cop}\",\"corrected\":false,\"correctable\":true,\"location\":{{\"start_line\":{line},\"start_column\":1,\"last_line\":{line},\"last_column\":2,\"length\":2,\"line\":{line},\"column\":1}}}}"
        )
    }

    fn batched() -> String {
        format!(
            "{{\"metadata\":{{\"rubocop_version\":\"1.91.0\",\"ruby_version\":\"3.4.2\"}},\"files\":[{{\"path\":\"a.rb\",\"offenses\":[{},{},{}]}},{{\"path\":\"b.rb\",\"offenses\":[]}},{{\"path\":\"c.rb\",\"offenses\":[{}]}}],\"summary\":{{\"offense_count\":4,\"target_file_count\":3,\"inspected_file_count\":3}}}}",
            offense("Style/Alpha", 1),
            offense("Lint/Syntax", 2),
            offense("Style/Beta", 3),
            offense("Style/Beta", 9),
        )
    }

    fn rules(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    /// A split report is byte for byte what `--only <that cop>` prints: the
    /// other requested cops' offenses are dropped, the whole file list and
    /// the metadata survive, and `offense_count` is recounted.
    #[test]
    fn split_matches_a_single_rule_report() {
        let split = split_by_rule(&batched(), &rules(&["Style/Alpha", "Style/Beta"])).unwrap();
        let names: Vec<&str> = split.iter().map(|(rule, _)| rule.as_str()).collect();
        assert_eq!(names, ["Style/Alpha", "Style/Beta"]);

        assert_eq!(
            split[0].1,
            format!(
                "{{\"metadata\":{{\"rubocop_version\":\"1.91.0\",\"ruby_version\":\"3.4.2\"}},\"files\":[{{\"path\":\"a.rb\",\"offenses\":[{},{}]}},{{\"path\":\"b.rb\",\"offenses\":[]}},{{\"path\":\"c.rb\",\"offenses\":[]}}],\"summary\":{{\"offense_count\":2,\"target_file_count\":3,\"inspected_file_count\":3}}}}",
                offense("Style/Alpha", 1),
                offense("Lint/Syntax", 2),
            )
        );
        assert_eq!(
            split[1].1,
            format!(
                "{{\"metadata\":{{\"rubocop_version\":\"1.91.0\",\"ruby_version\":\"3.4.2\"}},\"files\":[{{\"path\":\"a.rb\",\"offenses\":[{},{}]}},{{\"path\":\"b.rb\",\"offenses\":[]}},{{\"path\":\"c.rb\",\"offenses\":[{}]}}],\"summary\":{{\"offense_count\":3,\"target_file_count\":3,\"inspected_file_count\":3}}}}",
                offense("Lint/Syntax", 2),
                offense("Style/Beta", 3),
                offense("Style/Beta", 9),
            )
        );
    }

    /// `Lint/Syntax` is carried into every report only while it is an
    /// extra; requesting it explicitly keeps it to its own report, which is
    /// again what a single-rule run would have produced.
    #[test]
    fn requesting_the_auxiliary_cop_keeps_it_to_its_own_report() {
        let split =
            split_by_rule(&batched(), &rules(&["Style/Alpha", "Style/Beta", "Lint/Syntax"]))
                .unwrap();
        assert!(!split[0].1.contains("Lint/Syntax"));
        assert!(split[0].1.contains("\"offense_count\":1"));
        assert!(!split[1].1.contains("Lint/Syntax"));
        assert!(split[1].1.contains("\"offense_count\":2"));
        assert!(!split[2].1.contains("Style/Alpha"));
        assert!(split[2].1.contains("\"offense_count\":1"));
    }

    fn args(rule: &[&str], rules_file: Option<PathBuf>) -> ConformanceArgs {
        ConformanceArgs {
            app: PathBuf::from("."),
            rule: rules(rule),
            rules_file,
            defaults: false,
            refresh: false,
            no_write: true,
            truth_only: false,
        }
    }

    #[test]
    fn rules_file_extends_rule_flags_without_duplicates() {
        let path = std::env::temp_dir().join("xtask-conformance-rules-file.txt");
        fs::write(&path, "# a wave\nStyle/Beta\n\n  Lint/Gamma  # trailing note\nStyle/Alpha\n")
            .unwrap();
        let resolved = resolve_rules(&args(&["Style/Alpha"], Some(path.clone()))).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(resolved, rules(&["Style/Alpha", "Style/Beta", "Lint/Gamma"]));
    }

    #[test]
    fn an_empty_selection_is_rejected() {
        assert!(resolve_rules(&args(&[], None)).is_err());
    }
}
