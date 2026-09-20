//! `cargo xtask conformance-config`: compares `elysium config --format
//! show-cops` (per crates/cli's `config` subcommand contract) against a
//! ground-truth `rubocop --show-cops` capture, cop by cop.
//!
//! Ground-truth captures live under `/Users/paulo/Work/lab/corpus/<app>.show-cops.yml`,
//! produced by cloning a Rails application and running real RuboCop against
//! it (see `<app>.meta.txt` alongside each capture for how it was obtained).
//!
//! Both captures are the literal output of RuboCop's `--show-cops` flag: a
//! YAML mapping from cop name to that cop's resolved configuration, with
//! `# comment` lines (department headers, "Supports --autocorrect" markers)
//! interspersed. A YAML parser ignores those comments, so the file parses
//! directly as one mapping.
//!
//! Uses `saphyr` (the same YAML crate `config` picked) to parse both
//! documents as generic values, rather than deserializing into typed
//! structs, since RuboCop's per-cop config shape is cop-specific and this
//! tool only needs generic key/value comparison.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{bail, Context as _, Result};
use clap::Args;
use saphyr::{LoadableYamlNode as _, Yaml};

use crate::bench::{build_release_cli, workspace_root};

/// Cop departments that ship in separate RuboCop extension gems
/// (`rubocop-rails`, `rubocop-performance`, `rubocop-rspec`,
/// `rubocop-capybara`, `rubocop-factory_bot`, `rubocop-rake`) rather than
/// RuboCop core's `config/default.yml`. Elysium does not implement these
/// yet (Phase 6), so cops in these departments are skipped when comparing
/// against ground truth instead of counting as disagreements.
const EXTENSION_DEPARTMENTS: &[&str] =
    &["Rails", "Performance", "RSpec", "Capybara", "FactoryBot", "Rake"];

/// `cargo xtask conformance-config` arguments.
#[derive(Debug, Args)]
pub(crate) struct ConformanceConfigArgs {
    /// Application checkout whose `.rubocop.yml` chain to resolve config
    /// against (elysium's `config` subcommand is run with this as its
    /// working directory).
    #[arg(long, value_name = "PATH")]
    app: PathBuf,

    /// Ground-truth `rubocop --show-cops` capture to compare against.
    #[arg(long, value_name = "FILE")]
    truth: PathBuf,

    /// Print the comparison but always exit 0, even on disagreement.
    #[arg(long)]
    report_only: bool,
}

/// How a single (non-skipped) cop compared against ground truth.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CopOutcome {
    /// Every key, including `Enabled`, matches ground truth.
    Agree,
    /// The cop is present on only one side: RuboCop core added, removed, or
    /// renamed it between the embedded defaults' release and the release
    /// that produced the ground-truth capture. Not scored either way.
    VersionSkew,
    /// `Enabled` itself differs (this implies the cops disagree on "all
    /// keys" too, since `Enabled` is one of those keys).
    EnabledDiffers {
        /// `Enabled` as elysium reported it (`None` if absent or non-bool).
        actual: Option<bool>,
        /// `Enabled` as ground truth reported it.
        truth: Option<bool>,
    },
    /// `Enabled` agrees, but at least one other key differs.
    OtherKeyDiffers {
        /// Names of the keys (other than `Enabled`) that differ, sorted.
        keys: Vec<String>,
    },
}

/// One cop's comparison result, keyed by cop name.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CopDiff {
    name: String,
    outcome: CopOutcome,
}

/// The result of comparing two `--show-cops` captures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComparisonReport {
    /// Cops considered: present on both sides, not in a skipped extension
    /// department. This is the agreement denominator.
    total: usize,
    /// Of `total`, how many agree on `Enabled`.
    agree_enabled: usize,
    /// Of `total`, how many agree on every key.
    agree_all: usize,
    /// Ground-truth cops skipped because their department is a RuboCop
    /// extension gem elysium does not implement yet.
    skipped_extension: usize,
    /// Cops present on only one side (see [`CopOutcome::VersionSkew`]),
    /// excluded from `total`.
    version_skew: usize,
    /// Non-agreeing cops, in ground-truth order.
    diffs: Vec<CopDiff>,
}

impl ComparisonReport {
    /// True once every considered cop agrees on `Enabled` (the bar
    /// `conformance-config` enforces by default).
    fn full_enabled_agreement(&self) -> bool {
        self.total == 0 || self.agree_enabled == self.total
    }
}

/// Runs the `conformance-config` subcommand.
pub(crate) fn run(args: &ConformanceConfigArgs) -> Result<ExitCode> {
    let workspace_root = workspace_root();
    build_release_cli(&workspace_root)?;
    let binary = workspace_root.join("target/release/elysium");

    let truth = fs::read_to_string(&args.truth)
        .with_context(|| format!("reading ground-truth capture {}", args.truth.display()))?;

    let output = Command::new(&binary)
        .args(["config", "--format", "show-cops"])
        .current_dir(&args.app)
        .output()
        .with_context(|| {
            format!(
                "running `{} config --format show-cops` in {}",
                binary.display(),
                args.app.display()
            )
        })?;
    if !output.status.success() {
        bail!(
            "`elysium config --format show-cops` exited with {}:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let actual =
        String::from_utf8(output.stdout).context("elysium config output was not valid UTF-8")?;

    let report = compare(&actual, &truth, &args.app)?;
    print_report(&report);

    if report.full_enabled_agreement() || args.report_only {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::FAILURE)
    }
}

/// Prints per-cop diffs followed by the summary line.
fn print_report(report: &ComparisonReport) {
    for diff in &report.diffs {
        match &diff.outcome {
            CopOutcome::Agree => {}
            CopOutcome::VersionSkew => {
                println!("{}: version skew (present on one side only)", diff.name);
            }
            CopOutcome::EnabledDiffers { actual, truth } => println!(
                "{}: Enabled differs (elysium={}, truth={})",
                diff.name,
                format_bool_opt(*actual),
                format_bool_opt(*truth),
            ),
            CopOutcome::OtherKeyDiffers { keys } => {
                println!("{}: {} differs", diff.name, keys.join(", "));
            }
        }
    }
    println!(
        "{}/{} cops agree on Enabled, {}/{} agree on all keys ({} extension-department cops skipped, {} version skew)",
        report.agree_enabled,
        report.total,
        report.agree_all,
        report.total,
        report.skipped_extension,
        report.version_skew,
    );
}

/// Formats an optional bool for diff output, e.g. when `Enabled` is absent
/// or not a boolean.
fn format_bool_opt(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "<missing>",
    }
}

/// Compares two `--show-cops` YAML captures, skipping cops whose
/// department is a not-yet-implemented RuboCop extension (see
/// [`EXTENSION_DEPARTMENTS`]).
///
/// A cop present on only one side is [`CopOutcome::VersionSkew`] and is
/// excluded from the agreement totals: RuboCop core adds, removes, and
/// renames cops between releases, and `truth` and the embedded defaults are
/// rarely the same release. `Include`/`Exclude` lists are compared after
/// stripping `app_root` from every entry, since both captures absolutise
/// those patterns against the application checkout they were resolved
/// against.
fn compare(actual_yaml: &str, truth_yaml: &str, app_root: &Path) -> Result<ComparisonReport> {
    let actual_root = parse_show_cops(actual_yaml).context("parsing elysium's output")?;
    let truth_root = parse_show_cops(truth_yaml).context("parsing ground-truth capture")?;
    let actual_cops = actual_root
        .as_mapping()
        .context("elysium's output is not a YAML mapping of cop name to config")?;
    let truth_cops = truth_root
        .as_mapping()
        .context("ground-truth capture is not a YAML mapping of cop name to config")?;

    let mut total = 0;
    let mut agree_enabled = 0;
    let mut agree_all = 0;
    let mut skipped_extension = 0;
    let mut version_skew = 0;
    let mut diffs = Vec::new();

    for (name_yaml, truth_cop) in truth_cops {
        let name = name_yaml.as_str().context("ground-truth cop name key is not a string")?;
        if is_extension_cop(name) {
            skipped_extension += 1;
            continue;
        }

        let Some(actual_cop) = actual_cops.get(name_yaml) else {
            version_skew += 1;
            diffs.push(CopDiff { name: name.to_string(), outcome: CopOutcome::VersionSkew });
            continue;
        };
        total += 1;

        let truth_enabled = truth_cop.as_mapping_get("Enabled").and_then(Yaml::as_bool);
        let actual_enabled = actual_cop.as_mapping_get("Enabled").and_then(Yaml::as_bool);
        let outcome = if truth_enabled == actual_enabled {
            let keys = differing_keys(actual_cop, truth_cop, app_root);
            if keys.is_empty() {
                CopOutcome::Agree
            } else {
                CopOutcome::OtherKeyDiffers { keys }
            }
        } else {
            CopOutcome::EnabledDiffers { actual: actual_enabled, truth: truth_enabled }
        };

        match &outcome {
            CopOutcome::Agree => {
                agree_enabled += 1;
                agree_all += 1;
            }
            CopOutcome::OtherKeyDiffers { .. } => agree_enabled += 1,
            CopOutcome::EnabledDiffers { .. } | CopOutcome::VersionSkew => {}
        }
        diffs.push(CopDiff { name: name.to_string(), outcome });
    }

    // Cops elysium reports that ground truth's RuboCop release has never
    // heard of (added to the embedded defaults after `truth`'s release):
    // also version skew, and likewise excluded from `total`.
    for (name_yaml, _) in actual_cops {
        if truth_cops.contains_key(name_yaml) {
            continue;
        }
        let name = name_yaml.as_str().context("elysium's cop name key is not a string")?;
        if !is_extension_cop(name) {
            version_skew += 1;
        }
    }

    Ok(ComparisonReport { total, agree_enabled, agree_all, skipped_extension, version_skew, diffs })
}

/// Parses a `--show-cops` capture into its single root YAML document.
fn parse_show_cops(yaml: &str) -> Result<Yaml<'_>> {
    let mut docs = Yaml::load_from_str(yaml).context("scanning YAML")?;
    if docs.is_empty() {
        bail!("expected one YAML document, found none");
    }
    Ok(docs.remove(0))
}

/// True if `cop_name`'s department (the part before `/`) is a RuboCop
/// extension gem elysium does not implement yet.
fn is_extension_cop(cop_name: &str) -> bool {
    let department = cop_name.split('/').next().unwrap_or(cop_name);
    EXTENSION_DEPARTMENTS.contains(&department)
}

/// Returns the sorted names of top-level keys whose value differs between
/// `actual` and `truth` (both expected to be cop-config mappings). Excludes
/// `Enabled`, which callers compare separately.
///
/// `Include`/`Exclude` are compared with `app_root` stripped from every
/// entry first: both `--show-cops` runs absolutise these patterns against
/// whatever directory RuboCop/elysium started from, so a pattern that is
/// identical apart from its absolute prefix is not a real disagreement.
fn differing_keys(actual: &Yaml<'_>, truth: &Yaml<'_>, app_root: &Path) -> Vec<String> {
    let mut keys = BTreeSet::new();
    for cop in [actual, truth] {
        if let Some(mapping) = cop.as_mapping() {
            keys.extend(mapping.keys().filter_map(Yaml::as_str).map(str::to_string));
        }
    }
    keys.remove("Enabled");
    keys.retain(|key| {
        let actual_value = actual.as_mapping_get(key);
        let truth_value = truth.as_mapping_get(key);
        if key == "Include" || key == "Exclude" {
            normalized_paths(actual_value, app_root) != normalized_paths(truth_value, app_root)
        } else {
            actual_value != truth_value
        }
    });
    keys.into_iter().collect()
}

/// Strips `app_root`'s absolute-path prefix from every string entry of an
/// `Include`/`Exclude` list.
fn normalized_paths(value: Option<&Yaml<'_>>, app_root: &Path) -> Option<Vec<String>> {
    let root = app_root.to_string_lossy();
    let entries = value?.as_vec()?;
    Some(
        entries
            .iter()
            .map(|entry| {
                let text = entry.as_str().unwrap_or_default();
                text.strip_prefix(root.as_ref()).unwrap_or(text).to_string()
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::compare;

    /// Any root works for tests that do not exercise `Include`/`Exclude`
    /// normalization.
    const ROOT: &str = "/app";

    /// Ground truth exercising all four documented cases: `Style/Alpha`
    /// agrees fully, `Style/Beta` disagrees on `Enabled`, `Style/Gamma`
    /// agrees on `Enabled` but disagrees on `Description`, and
    /// `Rails/Delta` is a skipped extension-department cop.
    const TRUTH: &str = "\
Style/Alpha:
  Enabled: true
  Description: alpha cop
Style/Beta:
  Enabled: true
  Description: beta cop
Style/Gamma:
  Enabled: false
  Description: gamma cop
Rails/Delta:
  Enabled: true
  Description: rails specific
";

    const ACTUAL: &str = "\
Style/Alpha:
  Enabled: true
  Description: alpha cop
Style/Beta:
  Enabled: false
  Description: beta cop
Style/Gamma:
  Enabled: false
  Description: totally different
Rails/Delta:
  Enabled: false
  Description: does not matter, skipped
";

    #[test]
    fn agreeing_cop_counts_toward_both_tallies() {
        let report = compare(ACTUAL, TRUTH, Path::new(ROOT)).unwrap();
        let alpha = report.diffs.iter().find(|d| d.name == "Style/Alpha");
        // Fully agreeing cops are still recorded, just with `Agree`.
        assert_eq!(alpha.unwrap().outcome, super::CopOutcome::Agree);
    }

    #[test]
    fn enabled_mismatch_fails_both_tallies() {
        let report = compare(ACTUAL, TRUTH, Path::new(ROOT)).unwrap();
        let beta = report.diffs.iter().find(|d| d.name == "Style/Beta").unwrap();
        assert_eq!(
            beta.outcome,
            super::CopOutcome::EnabledDiffers { actual: Some(false), truth: Some(true) }
        );
    }

    #[test]
    fn other_key_mismatch_keeps_enabled_agreement() {
        let report = compare(ACTUAL, TRUTH, Path::new(ROOT)).unwrap();
        let gamma = report.diffs.iter().find(|d| d.name == "Style/Gamma").unwrap();
        assert_eq!(
            gamma.outcome,
            super::CopOutcome::OtherKeyDiffers { keys: vec!["Description".to_string()] }
        );
    }

    #[test]
    fn extension_department_cop_is_skipped_from_tallies() {
        let report = compare(ACTUAL, TRUTH, Path::new(ROOT)).unwrap();
        assert_eq!(report.skipped_extension, 1);
        assert!(!report.diffs.iter().any(|d| d.name == "Rails/Delta"));
    }

    #[test]
    fn summary_tallies_match_the_four_cases() {
        let report = compare(ACTUAL, TRUTH, Path::new(ROOT)).unwrap();
        // 3 considered cops (Rails/Delta skipped): Alpha and Gamma agree on
        // Enabled (2/3); only Alpha agrees on every key (1/3).
        assert_eq!(report.total, 3);
        assert_eq!(report.agree_enabled, 2);
        assert_eq!(report.agree_all, 1);
        assert!(!report.full_enabled_agreement());
    }

    #[test]
    fn cop_missing_from_actual_is_version_skew_not_disagreement() {
        let truth = "Style/Solo:\n  Enabled: true\n";
        let actual = "Style/Other:\n  Enabled: true\n";
        let report = compare(actual, truth, Path::new(ROOT)).unwrap();
        // Excluded from `total`, so it cannot fail `full_enabled_agreement`.
        assert_eq!(report.total, 0);
        assert_eq!(report.version_skew, 2);
        assert!(report.full_enabled_agreement());
        assert_eq!(report.diffs[0].outcome, super::CopOutcome::VersionSkew);
    }

    #[test]
    fn cop_missing_from_truth_counts_as_version_skew() {
        let truth = "Style/Solo:\n  Enabled: true\n";
        let actual = "Style/Solo:\n  Enabled: true\nStyle/NewerCop:\n  Enabled: true\n";
        let report = compare(actual, truth, Path::new(ROOT)).unwrap();
        assert_eq!(report.total, 1);
        assert_eq!(report.version_skew, 1);
        assert!(report.full_enabled_agreement());
    }

    #[test]
    fn normalized_paths_strips_shared_root_prefix() {
        use saphyr::{LoadableYamlNode as _, Yaml};

        let docs = Yaml::load_from_str("- /app/**/*.gemspec\n- /app/**/*_spec.rb\n").unwrap();
        let normalized = super::normalized_paths(Some(&docs[0]), Path::new("/app")).unwrap();
        assert_eq!(normalized, vec!["/**/*.gemspec", "/**/*_spec.rb"]);
    }

    #[test]
    fn include_exclude_agree_once_the_shared_root_is_stripped() {
        // Both `--show-cops` runs absolutise the pattern against the same
        // application root, so the literal strings already match here; the
        // interesting case is exercised at the `normalized_paths` level
        // above. This just confirms `compare` wires the root through.
        let truth = "Style/Solo:\n  Enabled: true\n  Exclude:\n  - /app/**/*.gemspec\n";
        let actual = "Style/Solo:\n  Enabled: true\n  Exclude:\n  - /app/**/*.gemspec\n";
        let report = compare(actual, truth, Path::new(ROOT)).unwrap();
        assert_eq!(report.agree_all, 1);
    }

    #[test]
    fn include_exclude_still_differs_beyond_the_shared_prefix() {
        let truth = "Style/Solo:\n  Enabled: true\n  Exclude:\n  - /app/**/*.gemspec\n";
        let actual = "Style/Solo:\n  Enabled: true\n  Exclude:\n  - /app/**/*.rb\n";
        let report = compare(actual, truth, Path::new(ROOT)).unwrap();
        let solo = report.diffs.iter().find(|d| d.name == "Style/Solo").unwrap();
        assert_eq!(
            solo.outcome,
            super::CopOutcome::OtherKeyDiffers { keys: vec!["Exclude".to_string()] }
        );
    }
}
