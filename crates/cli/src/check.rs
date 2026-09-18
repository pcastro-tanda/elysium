use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use anyhow::{Context as _, Result};
use config::FileMatcher;
use linter::Severity;
use rayon::prelude::*;
use ruby_source::{LineCol, SourceFile};

use crate::args::{CheckArgs, Format};
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
    pub nodes: u64,
    pub bytes: u64,
    pub io_errors: usize,
}

pub fn run(args: &CheckArgs) -> Result<ExitCode> {
    let started = Instant::now();
    if let Some(jobs) = args.jobs {
        rayon::ThreadPoolBuilder::new().num_threads(jobs).build_global().ok();
    }

    let root = std::env::current_dir().context("cannot determine working directory")?;
    let matcher = FileMatcher::rubocop_defaults();
    let files = discover::discover(
        &args.paths,
        &discover::Options { root: &root, matcher: &matcher, gitignore: !args.no_gitignore },
    )?;
    let discovered_at = Instant::now();

    let nodes = AtomicU64::new(0);
    let bytes = AtomicU64::new(0);
    let io_errors = AtomicUsize::new(0);

    let mut reports: Vec<FileReport> = files
        .par_iter()
        .map_init(rules::RuleSet::rubocop_defaults, |rule_set, path| {
            let display = path.strip_prefix(&root).unwrap_or(path).to_path_buf();
            let source = match SourceFile::read(path) {
                Ok(s) => s,
                Err(err) => {
                    io_errors.fetch_add(1, Ordering::Relaxed);
                    eprintln!("error: cannot read {}: {err}", path.display());
                    return FileReport { path: display, offenses: Vec::new() };
                }
            };
            bytes.fetch_add(source.bytes().len() as u64, Ordering::Relaxed);
            let result = linter::lint_file(&source, rule_set);
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
        })
        .collect();
    reports.sort_by(|a, b| a.path.cmp(&b.path));
    let linted_at = Instant::now();

    let summary = Summary {
        target_files: files.len(),
        inspected_files: files.len() - io_errors.load(Ordering::Relaxed),
        offenses: reports.iter().map(|r| r.offenses.len()).sum(),
        correctable: reports.iter().flat_map(|r| &r.offenses).filter(|o| o.correctable).count(),
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

fn char_len(bytes: &[u8]) -> u32 {
    u32::try_from(bytes.iter().filter(|&&b| (b & 0xC0) != 0x80).count()).unwrap_or(u32::MAX)
}

/// Display form of a report path.
pub fn display_path(path: &Path) -> String {
    path.display().to_string()
}
