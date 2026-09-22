//! `path:line:col: S: [Correctable] Cop/Name: message` lines followed by a
//! RuboCop-style summary line. Columns are 1-based like RuboCop's output.

use std::io::{self, Write};

use crate::check::{display_path, FileReport, Summary};

pub fn write(out: &mut impl Write, reports: &[FileReport], summary: &Summary) -> io::Result<()> {
    for report in reports {
        let path = display_path(&report.path);
        for o in &report.offenses {
            let correctable = if o.correctable { "[Correctable] " } else { "" };
            // Multi-line messages (RuboCop sometimes emits them) stay on one line.
            let message = o.message.replace('\n', " ");
            writeln!(
                out,
                "{path}:{}:{}: {}: {correctable}{}: {message}",
                o.start.line,
                o.start.column + 1,
                o.severity.code(),
                o.rule,
            )?;
        }
    }

    let files = plural(summary.inspected_files, "file", "files");
    let offenses = plural(summary.offenses, "offense", "offenses");
    write!(out, "\n{files} inspected, {offenses} detected")?;
    if summary.corrected > 0 {
        let n = plural(summary.corrected, "offense", "offenses");
        write!(out, ", {n} corrected")?;
    }
    if summary.correctable > 0 {
        let n = plural(summary.correctable, "offense", "offenses");
        write!(out, ", {n} autocorrectable")?;
    }
    writeln!(out)
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}
