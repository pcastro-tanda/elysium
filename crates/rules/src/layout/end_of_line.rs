//! `Layout/EndOfLine`, ported from RuboCop's
//! `lib/rubocop/cop/layout/end_of_line.rb`.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_source::Span;

/// RuboCop's `MSG_DETECTED`.
const MSG_DETECTED: &str = "Carriage return character detected.";
/// RuboCop's `MSG_MISSING`.
const MSG_MISSING: &str = "Carriage return character missing.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// CR+LF on Windows, LF elsewhere -- resolved once per run from the
    /// host OS the linter itself is running on, RuboCop's
    /// `Platform.windows?`.
    Native,
    /// LF on every platform.
    Lf,
    /// CR+LF on every platform.
    Crlf,
}

/// Checks for Windows-style line endings in the source code.
#[derive(Debug, Clone, Copy)]
pub struct EndOfLine {
    style: Style,
}

impl EndOfLine {
    /// RuboCop's `offense_message` resolved for the effective (non-`native`)
    /// style: whether `style` requires every line to end `\r\n`.
    fn wants_crlf(self) -> bool {
        match self.style {
            Style::Crlf => true,
            Style::Lf => false,
            Style::Native => cfg!(windows),
        }
    }

    /// RuboCop's `offense_message`: the message for one raw line (its text
    /// including whatever line terminator it has, or none for an
    /// unterminated final line), or `None` when the line already matches
    /// the effective style.
    fn offense_message(self, line: &[u8]) -> Option<&'static str> {
        if self.wants_crlf() {
            (!line.ends_with(b"\r\n")).then_some(MSG_MISSING)
        } else {
            (line.ends_with(b"\r\n") || line.ends_with(b"\r")).then_some(MSG_DETECTED)
        }
    }
}

impl Rule for EndOfLine {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EndOfLine",
        department: Department::Layout,
        summary: "Checks for Windows-style line endings in the source code.",
        explanation: "\
```ruby
# EnforcedStyle: native (default)
# The `native` style means that CR+LF (Carriage Return + Line Feed) is
# enforced on Windows, and LF is enforced on other platforms.

# bad
puts 'Hello' # Return character is LF on Windows.
puts 'Hello' # Return character is CR+LF on other than Windows.

# good
puts 'Hello' # Return character is CR+LF on Windows.
puts 'Hello' # Return character is LF on other than Windows.
```

```ruby
# EnforcedStyle: lf
# The `lf` style means that LF (Line Feed) is enforced on all platforms.

# bad
puts 'Hello' # Return character is CR+LF on all platforms.

# good
puts 'Hello' # Return character is LF on all platforms.
```

```ruby
# EnforcedStyle: crlf
# The `crlf` style means that CR+LF (Carriage Return + Line Feed) is
# enforced on all platforms.

# bad
puts 'Hello' # Return character is LF on all platforms.

# good
puts 'Hello' # Return character is CR+LF on all platforms.
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("native"),
            allowed: &["native", "lf", "crlf"],
            doc: "Which line-ending style to enforce.",
        }],
        blind_spots: "\
`native` resolves once from the host OS the linter itself runs on
(`cfg!(windows)`), matching RuboCop's `Platform.windows?` check against the
machine running the cop -- not `TargetRubyVersion` or any other per-project
setting. Only the line containing the file's last real token is scanned in
RuboCop, everything after (including a `__END__` data section) is ignored;
this port approximates that boundary with the line before `__END__` when
present, or the file's last line otherwise, so a trailing blank or
comment-only tail with a stray inconsistent terminator (never seen in
practice) would not reproduce RuboCop's exact cutoff.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "lf" => Style::Lf,
            "crlf" => Style::Crlf,
            _ => Style::Native,
        };
        Ok(Self { style })
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let bytes = ctx.source().bytes();
        let starts = ctx.source().lines().line_starts();

        // RuboCop's `last_line`: the line of the last real token, which
        // this port approximates as the line before `__END__` when the file
        // has a data section, or the file's last line otherwise.
        let scan_upper = match ctx.parsed().data_span() {
            Some(data) => ctx.line_col(data.start).line.saturating_sub(1),
            None => ctx.line_count(),
        };

        for line in 1..=scan_upper {
            let start = starts[(line - 1) as usize];
            let end = starts
                .get(line as usize)
                .copied()
                .unwrap_or_else(|| u32::try_from(bytes.len()).unwrap_or(u32::MAX));
            let raw_line = &bytes[start as usize..end as usize];

            let Some(message) = self.offense_message(raw_line) else { continue };

            // RuboCop's `unimportant_missing_cr?`: if there is no LF on the
            // (real) last line, we don't care that there's no CR either.
            if self.wants_crlf() && line == scan_upper && !raw_line.ends_with(b"\n") {
                continue;
            }

            ctx.report(&Self::META, Span::new(start, end), message);
            // Usually there will be carriage return characters on all or
            // none of the lines in a file, so we report only one offense.
            break;
        }
    }
}
