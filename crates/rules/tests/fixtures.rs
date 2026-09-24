//! Fixture harness: runs every `crates/rules/fixtures/<dept>/<snake>/*.rb`
//! case, which is a verbatim port of a RuboCop `expect_offense` example.
//!
//! One `#[test]` per department, so a failure lists every failing case in
//! that department at once. `cargo test -p rules --test fixtures -- layout`
//! runs one department. To narrow to a single cop directory, either set
//! `FIXTURE_COP=trailing_whitespace`, or pass the cop path as a second
//! filter -- `-- layout layout::trailing_whitespace` -- since libtest
//! matches test names by substring and would otherwise select nothing.
//!
//! `UPDATE_FIXTURES=1 cargo test -p rules --test fixtures` rewrites the
//! annotations in every `.rb` and every `.fixed.rb` from actual output, for
//! review -- never for blind acceptance.
//!
//! A case's sibling `<case>.offenses` file (one `Cop/Name:line` pair per
//! line) replays a RuboCop spec that built `Lint/RedundantCopDisableDirective`
//! with an explicit injected `offenses` array (diagnostics from other cops
//! that never actually ran in this investigation, see
//! `tools/port_spec.rb`'s `injected_offenses`): each pair becomes a synthetic
//! [`Diagnostic`] spanning that line, fed to `Rule::file_finish` through
//! `linter::lint_parsed_with_injected` without appearing in the case's own
//! expected output.

use config::{ConfigLoader, LoadedConfig};
use linter::{Diagnostic, FileSettings, RuleMeta};
use ruby_ast::{ParseOptions, Parsed, RubyVersion};
use ruby_source::SourceFile;
use rules::{RuleSet, ALL_RULES};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[test]
fn bundler() {
    run_department("bundler");
}

#[test]
fn gemspec() {
    run_department("gemspec");
}

#[test]
fn layout() {
    run_department("layout");
}

#[test]
fn lint() {
    run_department("lint");
}

#[test]
fn metrics() {
    run_department("metrics");
}

#[test]
fn migration() {
    run_department("migration");
}

#[test]
fn naming() {
    run_department("naming");
}

#[test]
fn security() {
    run_department("security");
}

#[test]
fn style() {
    run_department("style");
}

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn updating() -> bool {
    std::env::var_os("UPDATE_FIXTURES").is_some_and(|v| v != "0" && !v.is_empty())
}

/// `Layout/TrailingWhitespace` -> `("layout", "trailing_whitespace")`.
fn cop_path(meta: &RuleMeta) -> (String, String) {
    let (dept, name) = meta.name.split_once('/').expect("cop names are Dept/Name");
    (dept.to_ascii_lowercase(), snake_case(name))
}

fn snake_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 8);
    for (i, ch) in name.char_indices() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Cop-directory filter taken from the test binary's own arguments, so
/// `-- layout::trailing_whitespace` narrows the department test.
fn case_filter(dept: &str) -> Option<String> {
    if let Some(cop) = std::env::var_os("FIXTURE_COP") {
        return Some(cop.to_string_lossy().into_owned());
    }
    std::env::args()
        .skip(1)
        .filter(|arg| !arg.starts_with('-'))
        .find_map(|arg| arg.strip_prefix(dept)?.strip_prefix("::").map(str::to_string))
}

fn run_department(dept: &str) {
    let dir = fixtures_root().join(dept);
    if !dir.is_dir() {
        return;
    }
    let filter = case_filter(dept);

    let mut cop_dirs: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("read department directory")
        .map(|entry| entry.expect("read dir entry").path())
        .filter(|path| path.is_dir())
        .collect();
    cop_dirs.sort();

    let mut failures = Vec::new();
    for cop_dir in cop_dirs {
        let snake = cop_dir.file_name().unwrap_or_default().to_string_lossy().to_string();
        if let Some(filter) = &filter {
            if !snake.starts_with(filter.as_str()) {
                continue;
            }
        }
        let Some(meta) = ALL_RULES
            .iter()
            .copied()
            .find(|meta| cop_path(meta) == (dept.to_string(), snake.clone()))
        else {
            println!("note: {dept}/{snake}: no registered rule, skipping its fixtures");
            continue;
        };

        let mut cases: Vec<PathBuf> = std::fs::read_dir(&cop_dir)
            .expect("read cop directory")
            .map(|entry| entry.expect("read dir entry").path())
            .filter(|path| {
                path.extension().is_some_and(|ext| ext == "rb")
                    && !path.to_string_lossy().ends_with(".fixed.rb")
            })
            .collect();
        cases.sort();

        for case in cases {
            if let Err(message) = run_case(meta, &case) {
                failures.push(format!("{}\n{message}", case.display()));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} fixture case(s) failed in `{dept}`:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

/// One parsed annotation: the 1-based source line it belongs to, the 0-based
/// character column of its first caret, the caret count (0 for `^{}`), and
/// the message.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Annotation {
    line: u32,
    column: u32,
    length: u32,
    message: String,
}

/// Renders annotations the way RuboCop's `AnnotatedSource#to_s` does:
/// lines are the source split on `\n` (so the source's own final newline,
/// or lack of one, is preserved), annotations inserted after their line.
fn render(source: &[u8], annotations: &[Annotation]) -> String {
    let mut lines: Vec<String> = source
        .split(|&b| b == b'\n')
        .map(|line| String::from_utf8_lossy(line).into_owned())
        .collect();

    let mut sorted: Vec<&Annotation> = annotations.iter().collect();
    sorted.sort_by(|a, b| (a.line, &a.message, a.column).cmp(&(b.line, &b.message, b.column)));
    // Insert from the back so earlier insertion points stay valid.
    for annotation in sorted.into_iter().rev() {
        let carets = if annotation.length == 0 {
            "^{}".to_string()
        } else {
            "^".repeat(annotation.length as usize)
        };
        let text =
            format!("{}{carets} {}", " ".repeat(annotation.column as usize), annotation.message);
        let at = (annotation.line as usize).min(lines.len());
        lines.insert(at, text);
    }
    lines.join("\n")
}

/// RuboCop's `AnnotatedSource.parse`: separates annotation lines from
/// source lines, tracking which source line each annotation follows. The
/// file's trailing newline (or its absence) becomes the source's.
fn parse_annotated(bytes: &[u8]) -> (Vec<u8>, Vec<Annotation>) {
    let mut source_lines: Vec<&[u8]> = Vec::new();
    let mut annotations = Vec::new();

    for line in bytes.split(|&b| b == b'\n') {
        let text = String::from_utf8_lossy(line);
        let count = u32::try_from(source_lines.len()).unwrap_or(u32::MAX);
        match parse_annotation(&text, count) {
            Some(annotation) => annotations.push(annotation),
            None => source_lines.push(line),
        }
    }
    if source_lines.is_empty() {
        for annotation in &mut annotations {
            annotation.line = 1;
        }
    }
    (source_lines.join(&b'\n'), annotations)
}

/// RuboCop's `ANNOTATION_PATTERN`: optional leading whitespace, then either
/// a run of unescaped carets or `^{}`, then an optional single space and
/// the message.
fn parse_annotation(line: &str, previous_source_line: u32) -> Option<Annotation> {
    let indent = line.len() - line.trim_start().len();
    let rest = &line[indent..];
    if !line[..indent].chars().all(char::is_whitespace) {
        return None;
    }
    let column = u32::try_from(line[..indent].chars().count()).ok()?;

    let (length, after) = if let Some(after) = rest.strip_prefix("^{}") {
        (0u32, after)
    } else {
        let carets = rest.chars().take_while(|&c| c == '^').count();
        if carets == 0 {
            return None;
        }
        (u32::try_from(carets).ok()?, &rest[carets..])
    };
    let message = after.strip_prefix(' ').unwrap_or(after);
    Some(Annotation {
        line: previous_source_line,
        column,
        length,
        message: message.trim_end_matches('\n').trim_end_matches('\r').to_string(),
    })
}

fn load_config(case: &Path) -> LoadedConfig {
    let yml = case.with_extension("yml");
    let loader = ConfigLoader::new()
        .with_cwd(case.parent().expect("case has a parent"))
        .with_project_root(case.parent().expect("case has a parent"))
        .with_gem_lookup(false);
    if yml.is_file() {
        loader.load(Some(&yml)).expect("load fixture config")
    } else {
        loader.load(None).expect("load default config")
    }
}

fn ruby_version(target: Option<f32>) -> RubyVersion {
    match target {
        None => RubyVersion::Latest,
        Some(v) if v < 3.4 => RubyVersion::Ruby33,
        Some(v) if v < 3.5 => RubyVersion::Ruby34,
        Some(v) if v < 4.1 => RubyVersion::Ruby40,
        Some(_) => RubyVersion::Ruby41,
    }
}

fn to_annotations(source: &SourceFile, diagnostics: &[Diagnostic]) -> Vec<Annotation> {
    diagnostics
        .iter()
        .map(|d| {
            let start = source.line_col(d.span.start);
            let end = source.line_col(d.span.end);
            let length = if end.line == start.line {
                end.column - start.column
            } else {
                // RuboCop's carets stop at the end of the offense's first line.
                let line_len = char_count(source.line_text(start.line));
                line_len.saturating_sub(start.column)
            };
            Annotation {
                line: start.line,
                column: start.column,
                length,
                message: d.message.replace('\n', " "),
            }
        })
        .collect()
}

/// Characters (not bytes) in a byte slice, matching RuboCop's columns.
fn char_count(bytes: &[u8]) -> u32 {
    u32::try_from(bytes.iter().filter(|&&b| (b & 0xC0) != 0x80).count()).unwrap_or(u32::MAX)
}

/// The path the linted source claims to have: the `# file: NAME` comment on
/// the first line of the case's `.yml` (RuboCop's `expect_offense(src,
/// 'name.rb')` argument), else RuboCop's default buffer name `(string)`.
/// Cops such as `Lint/DuplicateMethods` embed it in their messages.
fn source_name(case: &Path) -> PathBuf {
    let name = std::fs::read_to_string(case.with_extension("yml")).ok().and_then(|yml| {
        yml.lines().next().and_then(|l| l.strip_prefix("# file: ")).map(str::to_owned)
    });
    PathBuf::from(name.unwrap_or_else(|| "(string)".to_string()))
}

/// Parses a case's sibling `<case>.offenses` file (one `Cop/Name:line` pair per line, see the
/// module docs) into `(rule, line)` pairs for [`linter::lint_parsed_with_injected`]. Cop names
/// go through [`linter::intern_rule_name`]'s bounded interner (the same one `config` uses for
/// every real cop name) rather than a one-off leak.
fn injected_offenses(case: &Path) -> Vec<(&'static str, u32)> {
    let Ok(text) = std::fs::read_to_string(case.with_extension("offenses")) else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (cop, line_no) =
                line.rsplit_once(':').unwrap_or_else(|| panic!("malformed offenses line: {line}"));
            let line_no: u32 = line_no.parse().unwrap_or_else(|_| panic!("bad line in: {line}"));
            (linter::intern_rule_name(cop), line_no)
        })
        .collect()
}

fn run_case(meta: &'static RuleMeta, case: &Path) -> Result<(), String> {
    let bytes = std::fs::read(case).map_err(|err| format!("cannot read case: {err}"))?;
    let (source_bytes, expected) = parse_annotated(&bytes);

    let cfg = load_config(case);
    let rule_set = RuleSet::only(&[meta.name], &cfg).map_err(|err| err.to_string())?;
    let options = ParseOptions {
        version: ruby_version(cfg.all_cops().target_ruby_version),
        partial_script: true,
    };
    let offenses = injected_offenses(case);

    let source = SourceFile::new(source_name(case), source_bytes.clone());
    let parsed = Parsed::parse_with(&source, options);
    if parsed.has_errors() {
        let messages: Vec<String> = parsed.errors().map(|e| e.message).collect();
        return Err(format!("  de-annotated source does not parse: {}", messages.join("; ")));
    }
    let mut lint_rules = rule_set.clone();
    let result = linter::lint_parsed_with_injected(
        &parsed,
        &mut lint_rules,
        &FileSettings::all_enabled(),
        &offenses,
    );
    let actual = to_annotations(&source, &result.diagnostics);

    let expected_text = render(&source_bytes, &expected);
    let actual_text = render(&source_bytes, &actual);

    if updating() && expected_text != actual_text {
        std::fs::write(case, actual_text.as_bytes()).map_err(|err| err.to_string())?;
    } else if expected_text != actual_text {
        return Err(format!(
            "  offenses differ.\n--- expected ---\n{}\n--- actual ---\n{}",
            indent(&expected_text),
            indent(&actual_text)
        ));
    }

    check_correction(meta, case, &source, options, &rule_set, &result.diagnostics, &offenses)
}

fn check_correction(
    meta: &'static RuleMeta,
    case: &Path,
    source: &SourceFile,
    options: ParseOptions,
    rule_set: &RuleSet,
    diagnostics: &[Diagnostic],
    offenses: &[(&'static str, u32)],
) -> Result<(), String> {
    if matches!(meta.fix, linter::FixAvailability::None) {
        return Ok(());
    }
    let fixed_path = PathBuf::from(format!("{}.fixed.rb", case.with_extension("").display()));
    let nofix_path = case.with_extension("nofix");
    // RuboCop's `expect_correction(..., loop: false)`: one pass, no
    // convergence loop.
    let single_pass = case.with_extension("singlepass").is_file();

    let (bytes, iterations, remaining, broke_syntax) = if single_pass {
        let (bytes, _) = linter::apply_fixes(source.bytes(), diagnostics, true);
        (bytes, 1, Vec::new(), false)
    } else {
        let mut fix_rules = rule_set.clone();
        let outcome = linter::fix_file_with_injected(
            source,
            options,
            &mut fix_rules,
            &FileSettings::all_enabled(),
            true,
            offenses,
        );
        let remaining: Vec<String> = outcome
            .diagnostics
            .iter()
            .filter(|d| d.fix.is_some())
            .map(|d| d.message.to_string())
            .collect();
        (
            outcome.bytes,
            outcome.report.iterations,
            remaining,
            outcome.report.introduced_syntax_error,
        )
    };

    if broke_syntax {
        return Err("  a fix made the source unparsable; it was rolled back".to_string());
    }

    if bytes == source.bytes() {
        if fixed_path.is_file() {
            return Err(format!("  nothing was corrected, but {} exists", fixed_path.display()));
        }
        return Ok(());
    }

    if nofix_path.is_file() {
        return Err(format!(
            "  case is marked `.nofix` but the fix engine changed it to:\n{}",
            indent(&String::from_utf8_lossy(&bytes))
        ));
    }

    let actual = String::from_utf8_lossy(&bytes).into_owned();
    if updating() {
        std::fs::write(&fixed_path, &bytes).map_err(|err| err.to_string())?;
    } else if let Ok(expected) = std::fs::read(&fixed_path) {
        if expected != bytes {
            return Err(format!(
                "  correction differs.\n--- expected ---\n{}\n--- actual ---\n{}",
                indent(&String::from_utf8_lossy(&expected)),
                indent(&actual)
            ));
        }
    }
    // No `.fixed.rb` and no `.nofix`: the spec asserted only the offenses,
    // so the correction is not compared -- but it still must converge.

    // Idempotence: nothing the rule can fix may survive the fix loop.
    if !remaining.is_empty() {
        return Err(format!(
            "  {} correctable offense(s) survived {iterations} fix iteration(s): {}",
            remaining.len(),
            remaining.join("; ")
        ));
    }
    Ok(())
}

fn indent(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let _ = writeln!(out, "  |{line}");
    }
    out
}
