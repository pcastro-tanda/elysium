use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context as _, Result};
use config::LoadedConfig;
use linter::Severity;
use rayon::prelude::*;
use ruby_ast::{ParseOptions, Parsed, RubyVersion};
use ruby_source::{LineCol, SourceFile};

use crate::args::{CheckArgs, Format};
use crate::config_load::load_config;
use crate::discover;
use crate::output;

/// One offense with positions already resolved, so the source buffer can be
/// dropped as soon as the file has been linted.
#[derive(Debug, Clone)]
pub struct Offense {
    pub rule: &'static str,
    pub message: String,
    pub severity: Severity,
    pub start: LineCol,
    /// Position of the exclusive end offset.
    pub end: LineCol,
    /// Length in characters.
    pub length: u32,
    pub correctable: bool,
}

/// Lint outcome for one file.
#[derive(Debug)]
pub struct FileReport {
    /// Path as given or discovered, relative to the working directory when possible.
    pub path: PathBuf,
    pub offenses: Vec<Offense>,
}

/// Aggregate counters shown by the summary and `--stats`.
#[derive(Debug, Default)]
pub struct Summary {
    pub target_files: usize,
    pub inspected_files: usize,
    pub offenses: usize,
    pub correctable: usize,
    /// Offenses `elysium fix` corrected. Always zero for `check`.
    pub corrected: usize,
    pub nodes: u64,
    pub bytes: u64,
    pub io_errors: usize,
}

/// Everything resolved once per invocation and shared by every worker
/// thread while linting files in parallel.
pub struct Session {
    pub cfg: LoadedConfig,
    pub root: PathBuf,
    pub overrides: Vec<CopOverride>,
    pub annotations: Arc<linter::Annotations>,
    pub parse_options: ParseOptions,
    /// The configured rules, cloned per file (rules keep per-file state).
    pub rule_set: rules::RuleSet,
}

/// Loads the configuration, prints its warnings, and precomputes the
/// per-file inputs every worker thread needs.
pub fn prepare(args: &CheckArgs) -> Result<Session> {
    let cwd = std::env::current_dir().context("cannot determine working directory")?;
    let cfg = load_config(args.config.as_deref(), args.no_config, &cwd)
        .map_err(|err| anyhow::anyhow!("{err}"))?;
    let root = cfg.root().to_path_buf();

    for warning in cfg.warnings() {
        eprintln!("warning: {warning}");
    }
    for extension in cfg.requested_extensions() {
        eprintln!("warning: `{extension}` is not supported yet; its cops use default settings");
    }

    let parse_options = ParseOptions {
        version: ruby_version(cfg.all_cops().target_ruby_version),
        partial_script: true,
    };
    let rule_set = select_rules(&cfg, &args.only, &args.except)?;
    let overrides = cop_overrides(&cfg, &args.only);
    let annotations = Arc::new(style_guide_annotations(&cfg));
    Ok(Session { cfg, root, overrides, annotations, parse_options, rule_set })
}

/// True when `selector` names `cop` exactly or names its department.
fn selects(selector: &str, cop: &str) -> bool {
    cop == selector
        || (cop.len() > selector.len()
            && cop.starts_with(selector)
            && cop.as_bytes()[selector.len()] == b'/')
}

/// Builds the rule set for this run, applying `--only`/`--except` on top
/// of the configuration. `--only` enables a cop the configuration
/// disabled, like RuboCop's.
fn select_rules(cfg: &LoadedConfig, only: &[String], except: &[String]) -> Result<rules::RuleSet> {
    if only.is_empty() && except.is_empty() {
        return rules::RuleSet::from_config(cfg).map_err(|err| anyhow::anyhow!("{err}"));
    }
    let names: Vec<&str> = rules::ALL_RULES
        .iter()
        .filter(|meta| {
            let selected = if only.is_empty() {
                cfg.cop(meta.name).map_or(meta.enabled_by_default, |cop| cop.enabled)
            } else {
                only.iter().any(|sel| selects(sel, meta.name))
            };
            selected && !except.iter().any(|sel| selects(sel, meta.name))
        })
        .map(|meta| meta.name)
        .collect();
    rules::RuleSet::only(&names, cfg).map_err(|err| anyhow::anyhow!("{err}"))
}

/// Builds the run-wide [`linter::Annotations`]: `AllCops`'s `DisplayStyleGuide`/
/// `ExtraDetails` switches, plus every registered cop's `StyleGuide`/
/// `References`/`Details` resolved from its configuration. Computed once
/// per run (this never varies per file) and shared with every worker
/// thread through [`Session::annotations`].
fn style_guide_annotations(cfg: &LoadedConfig) -> linter::Annotations {
    let all_cops = cfg.all_cops();
    let mut annotations =
        linter::Annotations::new(all_cops.display_style_guide, all_cops.extra_details);
    for meta in rules::ALL_RULES {
        if let Some(annotation) = cfg.style_guide_annotation(meta.name) {
            annotations.insert(meta.name, annotation);
        }
    }
    annotations
}

/// Reads, parses, and lints one file per the resolved [`Session`].
fn lint_one(
    session: &Session,
    path: &Path,
    io_errors: &AtomicUsize,
    bytes: &AtomicU64,
    nodes: &AtomicU64,
) -> FileReport {
    let display = path.strip_prefix(&session.root).unwrap_or(path).to_path_buf();
    let source = match SourceFile::read(path) {
        Ok(s) => s,
        Err(err) => {
            io_errors.fetch_add(1, Ordering::Relaxed);
            eprintln!("error: cannot read {}: {err}", path.display());
            return FileReport { path: display, offenses: Vec::new() };
        }
    };
    bytes.fetch_add(source.bytes().len() as u64, Ordering::Relaxed);
    let parsed = Parsed::parse_with(&source, session.parse_options);
    let settings = file_settings(&session.cfg, &session.overrides, &display, &session.annotations);
    let mut rules = session.rule_set.clone();
    let result = linter::lint_parsed_with(&parsed, &mut rules, &settings);
    nodes.fetch_add(u64::from(result.node_count), Ordering::Relaxed);
    let offenses = result
        .diagnostics
        .into_iter()
        .map(|d| {
            let start = source.line_col(d.span.start);
            let end = source.line_col(d.span.end);
            let length = char_len(source.slice(d.span));
            Offense {
                rule: d.rule,
                message: d.message.into_owned(),
                severity: d.severity,
                start,
                end,
                length,
                correctable: d.fix.is_some(),
            }
        })
        .collect();
    FileReport { path: display, offenses }
}

pub fn run(args: &CheckArgs) -> Result<ExitCode> {
    let started = Instant::now();
    if let Some(jobs) = args.jobs {
        rayon::ThreadPoolBuilder::new().num_threads(jobs).build_global().ok();
    }

    let session = prepare(args)?;
    let files = discover::discover(
        &args.paths,
        &discover::Options {
            root: &session.root,
            matcher: session.cfg.file_matcher(),
            gitignore: !args.no_gitignore,
        },
    )?;
    let discovered_at = Instant::now();

    let nodes = AtomicU64::new(0);
    let bytes = AtomicU64::new(0);
    let io_errors = AtomicUsize::new(0);

    let mut reports: Vec<FileReport> =
        files.par_iter().map(|path| lint_one(&session, path, &io_errors, &bytes, &nodes)).collect();
    reports.sort_by(|a, b| a.path.cmp(&b.path));
    let linted_at = Instant::now();

    let summary = Summary {
        target_files: files.len(),
        inspected_files: files.len() - io_errors.load(Ordering::Relaxed),
        offenses: reports.iter().map(|r| r.offenses.len()).sum(),
        correctable: reports.iter().flat_map(|r| &r.offenses).filter(|o| o.correctable).count(),
        corrected: 0,
        nodes: nodes.load(Ordering::Relaxed),
        bytes: bytes.load(Ordering::Relaxed),
        io_errors: io_errors.load(Ordering::Relaxed),
    };

    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    match args.format {
        Format::Human => output::human::write(&mut out, &reports, &summary)?,
        Format::Json => output::json::write(&mut out, &reports, &summary)?,
    }
    out.flush()?;

    if args.stats {
        let total = started.elapsed();
        eprintln!(
            "files: {} ({} bytes)  nodes: {}  discover: {:.1?}  lint: {:.1?}  output: {:.1?}  total: {:.1?}  threads: {}",
            summary.inspected_files,
            summary.bytes,
            summary.nodes,
            discovered_at - started,
            linted_at - discovered_at,
            linted_at.elapsed(),
            total,
            rayon::current_num_threads(),
        );
    }

    Ok(if summary.io_errors > 0 {
        ExitCode::from(2)
    } else if summary.offenses > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

/// A cop whose enablement or severity can vary from file to file: disabled
/// by default, restricted by an `Include`/`Exclude` list, or given a
/// severity override. Every cop absent from this list runs enabled at its
/// default severity for every file, so [`file_settings`] only walks this
/// (much smaller) list per file instead of every known cop.
pub struct CopOverride {
    name: &'static str,
    severity: Option<Severity>,
    /// Named by `--only`: runs even when the configuration disables it,
    /// but still honours its `Include`/`Exclude`.
    forced: bool,
}

/// Precomputes [`CopOverride`]s once per run so per-file settings building
/// stays cheap.
fn cop_overrides(cfg: &LoadedConfig, only: &[String]) -> Vec<CopOverride> {
    cfg.cops()
        .filter(|(_, cop)| {
            !cop.enabled
                || !cop.include.is_empty()
                || !cop.exclude.is_empty()
                || cop.severity.is_some()
        })
        .map(|(name, cop)| CopOverride {
            name: linter::intern_rule_name(name),
            severity: cop.severity,
            forced: only.iter().any(|sel| selects(sel, name)),
        })
        .collect()
}

/// Builds the [`linter::FileSettings`] for one file: cops absent from
/// `overrides` stay enabled at their default severity; cops present are
/// re-checked against [`LoadedConfig::is_cop_enabled_for`] (per-cop
/// `Include`/`Exclude` for `relative`; `AllCops`'s `Include`/`Exclude` was
/// already applied once, at discovery, when `relative` was selected as a
/// target). `annotations` is the run-wide style-guide/reference/details
/// table built once by [`style_guide_annotations`] and shared unchanged.
pub fn file_settings(
    cfg: &LoadedConfig,
    overrides: &[CopOverride],
    relative: &Path,
    annotations: &Arc<linter::Annotations>,
) -> linter::FileSettings {
    let mut settings = linter::FileSettings::all_enabled();
    settings.set_annotations(Arc::clone(annotations));
    for over in overrides {
        let enabled = if over.forced {
            cfg.is_cop_targeting(over.name, relative)
        } else {
            cfg.is_cop_enabled_for(over.name, relative)
        };
        if enabled {
            if let Some(severity) = over.severity {
                settings.set_severity(over.name, severity);
            }
        } else {
            settings.disable(over.name);
        }
    }
    settings
}

/// Maps `AllCops/TargetRubyVersion` to the closest syntax version Prism
/// models (Prism only understands 3.3 and newer).
fn ruby_version(target: Option<f32>) -> RubyVersion {
    let Some(version) = target else { return RubyVersion::Latest };
    if version < 3.4 {
        RubyVersion::Ruby33
    } else if version < 3.5 {
        RubyVersion::Ruby34
    } else if version < 4.1 {
        RubyVersion::Ruby40
    } else {
        RubyVersion::Ruby41
    }
}

pub fn char_len(bytes: &[u8]) -> u32 {
    u32::try_from(bytes.iter().filter(|&&b| (b & 0xC0) != 0x80).count()).unwrap_or(u32::MAX)
}

/// Display form of a report path.
pub fn display_path(path: &Path) -> String {
    path.display().to_string()
}
