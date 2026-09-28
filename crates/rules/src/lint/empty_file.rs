//! `Lint/EmptyFile`, ported from RuboCop's `lib/rubocop/cop/lint/empty_file.rb`.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};

/// RuboCop's `MSG`.
const MSG: &str = "Empty file detected.";

/// Enforces that Ruby source files are not empty.
#[derive(Debug, Clone)]
pub struct EmptyFile {
    allow_comments: bool,
}

impl Rule for EmptyFile {
    const META: RuleMeta = RuleMeta {
        name: "Lint/EmptyFile",
        department: Department::Lint,
        summary: "Enforces that Ruby source files are not empty.",
        explanation: "\
Enforces that Ruby source files are not empty.

```ruby
# bad
# Empty file

# good
# File containing non commented source lines
```

```ruby
# AllowComments: true (default)
# good
# File consisting only of comments
```

```ruby
# AllowComments: false
# bad
# File consisting only of comments
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[],
        config: &[ConfigOption {
            name: "AllowComments",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Allow files that consist only of comments (and blank lines).",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_comments: options.bool("AllowComments") })
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        if self.is_offending(ctx) {
            ctx.report_global(&Self::META, MSG);
        }
    }
}

impl EmptyFile {
    /// RuboCop's `offending?`.
    fn is_offending(&self, ctx: &Context<'_>) -> bool {
        ctx.source().bytes().is_empty() || (!self.allow_comments && contains_only_comments(ctx))
    }
}

/// RuboCop's `contains_only_comments?`: every line, up to a possible
/// `__END__` data section, is either blank (`String#blank?`: empty or all
/// whitespace once left-stripped) or a comment line (`Util#comment_line?`:
/// `#` as its first non-whitespace character).
fn contains_only_comments(ctx: &Context<'_>) -> bool {
    let last_line = match ctx.parsed().data_span() {
        Some(data) => ctx.line_col(data.start).line.saturating_sub(1),
        None => ctx.line_count(),
    };
    ctx.lines().take_while(|&(line, _)| line <= last_line).all(|(_, span)| {
        let text = ctx.text(span);
        is_blank(text) || is_comment_line(text)
    })
}

/// Ruby's (non-Unicode) whitespace class RuboCop's `blank?`/`comment_line?`
/// operate over: space, tab, newline, vertical tab, form feed, carriage
/// return.
fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

/// RuboCop's `String#blank?` core extension: `empty? || lstrip.empty?`.
fn is_blank(text: &[u8]) -> bool {
    text.iter().all(|&b| is_space(b))
}

/// `Util#comment_line?`: `/^\s*#/.match?(line_source)`.
fn is_comment_line(text: &[u8]) -> bool {
    text.iter().find(|&&b| !is_space(b)) == Some(&b'#')
}
