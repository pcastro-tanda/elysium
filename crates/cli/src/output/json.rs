//! RuboCop's JSON formatter schema, byte-for-byte compatible in shape so
//! existing tooling (editors, CI parsers, `rubocop-junit`-style converters)
//! can consume it unchanged.

use std::io::{self, Write};

use serde::Serialize;

use crate::check::{display_path, FileReport, Summary};

#[derive(Serialize)]
struct Root<'a> {
    metadata: Metadata<'a>,
    files: Vec<File<'a>>,
    summary: SummaryJson,
}

#[derive(Serialize)]
struct Metadata<'a> {
    rubocop_version: &'a str,
    ruby_engine: &'a str,
    ruby_version: &'a str,
    ruby_patchlevel: &'a str,
    ruby_platform: &'a str,
}

#[derive(Serialize)]
struct File<'a> {
    path: String,
    offenses: Vec<Offense<'a>>,
}

#[derive(Serialize)]
struct Offense<'a> {
    severity: &'static str,
    message: &'a str,
    cop_name: &'static str,
    corrected: bool,
    correctable: bool,
    location: Location,
}

#[derive(Serialize)]
struct Location {
    start_line: u32,
    start_column: u32,
    last_line: u32,
    last_column: u32,
    length: u32,
    line: u32,
    column: u32,
}

#[derive(Serialize)]
// Field names are RuboCop's JSON schema.
#[allow(clippy::struct_field_names)]
struct SummaryJson {
    offense_count: usize,
    target_file_count: usize,
    inspected_file_count: usize,
}

pub fn write(out: &mut impl Write, reports: &[FileReport], summary: &Summary) -> io::Result<()> {
    let root = Root {
        metadata: Metadata {
            // Reported as the RuboCop release whose defaults and message
            // texts this build tracks, so consumers keying on it behave.
            rubocop_version: crate::output::RUBOCOP_COMPAT_VERSION,
            ruby_engine: "elysium",
            ruby_version: env!("CARGO_PKG_VERSION"),
            ruby_patchlevel: "0",
            ruby_platform: std::env::consts::ARCH,
        },
        files: reports
            .iter()
            .map(|r| File {
                path: display_path(&r.path),
                offenses: r
                    .offenses
                    .iter()
                    .map(|o| Offense {
                        severity: o.severity.name(),
                        message: &o.message,
                        cop_name: o.rule,
                        corrected: false,
                        correctable: o.correctable,
                        location: Location {
                            start_line: o.start.line,
                            start_column: o.start.column + 1,
                            last_line: o.end.line,
                            // RuboCop reports the 0-based column of the end
                            // position, clamped to 1 when it is 0.
                            last_column: o.end.column.max(1),
                            length: o.length,
                            line: o.start.line,
                            column: o.start.column + 1,
                        },
                    })
                    .collect(),
            })
            .collect(),
        summary: SummaryJson {
            offense_count: summary.offenses,
            target_file_count: summary.target_files,
            inspected_file_count: summary.inspected_files,
        },
    };
    serde_json::to_writer(&mut *out, &root).map_err(io::Error::other)?;
    writeln!(out)
}
