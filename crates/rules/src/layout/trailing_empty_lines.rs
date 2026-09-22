//! `Layout/TrailingEmptyLines`, ported from RuboCop's
//! `lib/rubocop/cop/layout/trailing_empty_lines.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// Exactly one final newline, no trailing blank line.
    FinalNewline,
    /// Exactly one trailing blank line before the final newline.
    FinalBlankLine,
}

/// Looks for trailing blank lines and a final newline in the source code.
#[derive(Debug, Clone)]
pub struct TrailingEmptyLines {
    style: Style,
}

impl Rule for TrailingEmptyLines {
    const META: RuleMeta = RuleMeta {
        name: "Layout/TrailingEmptyLines",
        department: Department::Layout,
        summary: "Checks trailing blank lines and final newline.",
        explanation: "\
A file should end with exactly one newline, with no extra blank lines before
it (or, under `EnforcedStyle: final_blank_line`, exactly one blank line
before the final newline).

```ruby
# EnforcedStyle: final_newline (default)

# bad
class Foo; end

# EOF

# bad
class Foo; end # EOF

# good
class Foo; end
# EOF
```

```ruby
# EnforcedStyle: final_blank_line

# bad
class Foo; end
# EOF

# bad
class Foo; end # EOF

# good
class Foo; end

# EOF
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("final_newline"),
            allowed: &["final_newline", "final_blank_line"],
            doc: "The blank-line convention required at the end of a file.",
        }],
        blind_spots: "\
`__END__` detection is RuboCop's own heuristic: any occurrence of the literal
text `__END__` anywhere in the file (even inside a string or comment)
suppresses the cop, matching RuboCop's `buffer.source.match?(/\\s*__END__/)`
check exactly (its token-based fallback is unreachable dead code in RuboCop
itself, since anything the fallback could find is already a substring of
`buffer.source` and so would already have matched the first check).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "final_blank_line" => Style::FinalBlankLine,
            _ => Style::FinalNewline,
        };
        Ok(Self { style })
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let source = ctx.source().bytes();
        if source.is_empty() {
            return;
        }
        // RuboCop's `ends_in_end?`: a `__END__` data section may need a
        // particular trailing newline count for its own reasons, so skip
        // the file entirely if the literal text appears anywhere.
        if contains_end_marker(source) {
            return;
        }
        // RuboCop's `end_with_percent_blank_string?`.
        if source.ends_with(b"%\n\n") {
            return;
        }

        let ws_len = trailing_whitespace_len(source);
        let whitespace_at_end = &source[source.len() - ws_len..];
        let blank_lines = i64::try_from(count_newlines(whitespace_at_end)).unwrap_or(0) - 1;
        let wanted_blank_lines = i64::from(self.style == Style::FinalBlankLine);
        if blank_lines == wanted_blank_lines {
            return;
        }

        let source_len = u32::try_from(source.len()).unwrap_or(u32::MAX);
        let mut begin_pos = source_len - u32::try_from(ws_len).unwrap_or(0);
        let autocorrect_range = Span::new(begin_pos, source_len);
        if ws_len != 0 {
            begin_pos += 1;
        }
        let report_range = Span::new(begin_pos, source_len);

        let message = message_for(blank_lines, wanted_blank_lines);
        let replacement: &[u8] = if self.style == Style::FinalNewline { b"\n" } else { b"\n\n" };
        ctx.report_with_fix(
            &Self::META,
            report_range,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(autocorrect_range, replacement)],
            },
        );
    }
}

/// RuboCop's `buffer.source.match?(/\s*__END__/)`: true when the literal
/// text `__END__` appears anywhere in the source.
fn contains_end_marker(source: &[u8]) -> bool {
    const MARKER: &[u8] = b"__END__";
    source.windows(MARKER.len()).any(|w| w == MARKER)
}

/// Length of the maximal contiguous run of Ruby's `\s` bytes
/// (` \t\r\n\f\v`) at the end of `source`, matching Ruby's `/\s*\Z/`
/// applied to `buffer.source` (an unanchored greedy `\s*` followed by
/// `\Z` always finds this exact run).
fn trailing_whitespace_len(source: &[u8]) -> usize {
    source.iter().rev().take_while(|&&b| is_ruby_space(b)).count()
}

/// Ruby's `\s` character class (ASCII-only, not full Unicode whitespace).
fn is_ruby_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0x0C | 0x0B)
}

/// Number of `\n` bytes in `bytes` (RuboCop's `String#count("\n")`).
fn count_newlines(bytes: &[u8]) -> usize {
    let mut count = 0;
    for &b in bytes {
        if b == b'\n' {
            count += 1;
        }
    }
    count
}

/// RuboCop's `message`.
fn message_for(blank_lines: i64, wanted_blank_lines: i64) -> String {
    match blank_lines {
        -1 => "Final newline missing.".to_string(),
        0 => "Trailing blank line missing.".to_string(),
        n => {
            let instead_of = if wanted_blank_lines == 0 {
                String::new()
            } else {
                format!("instead of {wanted_blank_lines} ")
            };
            format!("{n} trailing blank lines {instead_of}detected.")
        }
    }
}
