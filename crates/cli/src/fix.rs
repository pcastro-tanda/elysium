//! `elysium fix`: applies the fixes rules attach to their offenses, then
//! reports what is left exactly like `check` does.
//!
//! Files are written atomically (temporary file in the same directory plus
//! a rename) and only when their bytes actually changed, so a fix run over
//! a clean tree touches nothing.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use anyhow::{Context as _, Result};
use rayon::prelude::*;
use ruby_source::SourceFile;

use crate::args::{FixArgs, Format};
use crate::check::{
    char_len, display_path, file_settings, prepare, FileReport, Offense, Session, Summary,
};
use crate::discover;
use crate::output;

/// One fixed file: the offenses left over, and the diff when `--diff` was
/// requested and the file would change.
struct FileFix {
    report: FileReport,
    diff: Option<String>,
    corrected: usize,
}

pub fn run(args: &FixArgs) -> Result<ExitCode> {
    if let Some(jobs) = args.check.jobs {
        rayon::ThreadPoolBuilder::new().num_threads(jobs).build_global().ok();
    }

    let session = prepare(&args.check)?;
    let files = discover::discover(
        &args.check.paths,
        &discover::Options {
            root: &session.root,
            matcher: session.cfg.file_matcher(),
            gitignore: !args.check.no_gitignore,
        },
    )?;

    let bytes = AtomicU64::new(0);
    let io_errors = AtomicUsize::new(0);

    let mut results: Vec<FileFix> =
        files.par_iter().map(|path| fix_one(&session, args, path, &io_errors, &bytes)).collect();
    results.sort_by(|a, b| a.report.path.cmp(&b.report.path));

    let summary = Summary {
        target_files: files.len(),
        inspected_files: files.len() - io_errors.load(Ordering::Relaxed),
        offenses: results.iter().map(|r| r.report.offenses.len()).sum(),
        correctable: results
            .iter()
            .flat_map(|r| &r.report.offenses)
            .filter(|o| o.correctable)
            .count(),
        corrected: results.iter().map(|r| r.corrected).sum(),
        nodes: 0,
        bytes: bytes.load(Ordering::Relaxed),
        io_errors: io_errors.load(Ordering::Relaxed),
    };

    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    if args.diff {
        for result in &results {
            if let Some(diff) = &result.diff {
                out.write_all(diff.as_bytes())?;
            }
        }
    }
    let reports: Vec<FileReport> = results.into_iter().map(|r| r.report).collect();
    match args.check.format {
        Format::Human => output::human::write(&mut out, &reports, &summary)?,
        Format::Json => output::json::write(&mut out, &reports, &summary)?,
    }
    out.flush()?;

    Ok(if summary.io_errors > 0 {
        ExitCode::from(2)
    } else if summary.offenses > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn fix_one(
    session: &Session,
    args: &FixArgs,
    path: &Path,
    io_errors: &AtomicUsize,
    bytes: &AtomicU64,
) -> FileFix {
    let display = path.strip_prefix(&session.root).unwrap_or(path).to_path_buf();
    let empty = |report| FileFix { report, diff: None, corrected: 0 };
    let source = match SourceFile::read(path) {
        Ok(source) => source,
        Err(err) => {
            io_errors.fetch_add(1, Ordering::Relaxed);
            eprintln!("error: cannot read {}: {err}", path.display());
            return empty(FileReport { path: display, offenses: Vec::new() });
        }
    };
    bytes.fetch_add(source.bytes().len() as u64, Ordering::Relaxed);

    let settings = file_settings(&session.cfg, &session.overrides, &display, &session.annotations);
    let mut rules = session.rule_set.clone();
    let outcome =
        linter::fix_file(&source, session.parse_options, &mut rules, &settings, args.unsafe_fixes);
    if outcome.report.introduced_syntax_error {
        eprintln!(
            "warning: a fix made {} unparsable; its corrections were rolled back",
            path.display()
        );
    }

    let changed = outcome.bytes != source.bytes();
    let mut diff = None;
    if changed {
        if args.diff {
            diff = Some(unified_diff(&display, source.bytes(), &outcome.bytes));
        } else if let Err(err) = write_atomically(path, &outcome.bytes) {
            io_errors.fetch_add(1, Ordering::Relaxed);
            eprintln!("error: cannot write {}: {err:#}", path.display());
        }
    }

    // Offense positions refer to the fixed source.
    let fixed = SourceFile::new(path.to_path_buf(), outcome.bytes);
    let offenses = outcome
        .diagnostics
        .into_iter()
        .map(|d| {
            let start = fixed.line_col(d.span.start);
            let end = fixed.line_col(d.span.end);
            Offense {
                rule: d.rule,
                message: d.message.into_owned(),
                severity: d.severity,
                start,
                end,
                length: char_len(fixed.slice(d.span)),
                correctable: d.fix.is_some(),
            }
        })
        .collect();

    FileFix {
        report: FileReport { path: display, offenses },
        diff,
        corrected: outcome.report.applied,
    }
}

/// Writes `contents` to `path` through a temporary file in the same
/// directory, so a crash mid-write cannot truncate the original.
fn write_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let temporary: PathBuf = directory.join(format!(".{name}.elysium-{}", std::process::id()));
    std::fs::write(&temporary, contents)
        .with_context(|| format!("writing {}", temporary.display()))?;
    if let Err(err) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(err).with_context(|| format!("replacing {}", path.display()));
    }
    Ok(())
}

/// A unified diff of one file's correction.
///
/// Fixes are byte edits that rarely change the line count, so equal-length
/// inputs are compared line by line and grouped into hunks with three
/// lines of context. When the line count does change, the diff falls back
/// to a single hunk over the region between the common prefix and suffix.
fn unified_diff(path: &Path, old: &[u8], new: &[u8]) -> String {
    let old_lines: Vec<String> = split_lines(old);
    let new_lines: Vec<String> = split_lines(new);
    let display = display_path(path);

    // Absolute paths (no configuration root to strip against) are printed
    // as-is rather than glued onto `a/`.
    let (old_label, new_label) = if Path::new(&display).is_absolute() {
        (display.clone(), display.clone())
    } else {
        (format!("a/{display}"), format!("b/{display}"))
    };
    let mut out = String::new();
    let _ = writeln!(out, "--- {old_label}");
    let _ = writeln!(out, "+++ {new_label}");

    if old_lines.len() == new_lines.len() {
        let mut index = 0usize;
        while index < old_lines.len() {
            if old_lines[index] == new_lines[index] {
                index += 1;
                continue;
            }
            let start = index;
            let mut end = index;
            while end < old_lines.len() {
                if old_lines[end] == new_lines[end] {
                    // Keep going only while another change is within the
                    // context window, so nearby edits share a hunk.
                    let next = (end..(end + 3).min(old_lines.len()))
                        .find(|&i| old_lines[i] != new_lines[i]);
                    match next {
                        Some(i) => end = i,
                        None => break,
                    }
                }
                end += 1;
            }
            let from = start.saturating_sub(3);
            let to = (end + 3).min(old_lines.len());
            write_hunk(&mut out, &old_lines[from..to], &new_lines[from..to], from);
            index = to;
        }
        return out;
    }

    let prefix = old_lines.iter().zip(&new_lines).take_while(|(a, b)| a == b).count();
    let suffix = old_lines[prefix..]
        .iter()
        .rev()
        .zip(new_lines[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let from = prefix.saturating_sub(3);
    let old_to = old_lines.len() - suffix;
    let new_to = new_lines.len() - suffix;
    let context = 3.min(suffix);
    write_hunk(
        &mut out,
        &old_lines[from..old_to + context],
        &new_lines[from..new_to + context],
        from,
    );
    out
}

/// Writes one hunk. Equal-length sides are walked line by line so
/// unchanged lines inside the hunk stay context; otherwise the differing
/// region is emitted as removals followed by additions.
fn write_hunk(out: &mut String, old: &[String], new: &[String], start: usize) {
    let _ = writeln!(out, "@@ -{},{} +{},{} @@", start + 1, old.len(), start + 1, new.len());
    if old.len() == new.len() {
        let mut index = 0usize;
        while index < old.len() {
            if old[index] == new[index] {
                let _ = writeln!(out, " {}", old[index]);
                index += 1;
                continue;
            }
            let run_start = index;
            while index < old.len() && old[index] != new[index] {
                index += 1;
            }
            for line in &old[run_start..index] {
                let _ = writeln!(out, "-{line}");
            }
            for line in &new[run_start..index] {
                let _ = writeln!(out, "+{line}");
            }
        }
        return;
    }

    let lead = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let tail =
        old[lead..].iter().rev().zip(new[lead..].iter().rev()).take_while(|(a, b)| a == b).count();
    for line in &old[..lead] {
        let _ = writeln!(out, " {line}");
    }
    for line in &old[lead..old.len() - tail] {
        let _ = writeln!(out, "-{line}");
    }
    for line in &new[lead..new.len() - tail] {
        let _ = writeln!(out, "+{line}");
    }
    for line in &old[old.len() - tail..] {
        let _ = writeln!(out, " {line}");
    }
}

fn split_lines(bytes: &[u8]) -> Vec<String> {
    bytes.split(|&b| b == b'\n').map(|line| String::from_utf8_lossy(line).into_owned()).collect()
}
