//! `Layout/TrailingWhitespace`, ported from RuboCop's
//! `lib/rubocop/cop/layout/trailing_whitespace.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::{char_len, Span};

/// RuboCop's `MSG`.
const MSG: &str = "Trailing whitespace detected.";

/// One heredoc body found in the file: the lines its body spans (RuboCop's
/// `body.first_line...body.last_line`, i.e. `end_line` exclusive), the
/// body's indentation level, and whether the heredoc is single-quoted (no
/// interpolation, so trailing whitespace cannot be preserved with `#{}`).
#[derive(Debug, Clone, Copy)]
struct HeredocBody {
    first_line: u32,
    end_line: u32,
    indent: u32,
    is_static: bool,
}

/// Looks for trailing whitespace in the source code.
#[derive(Debug, Clone)]
pub struct TrailingWhitespace {
    allow_in_heredoc: bool,
    /// Heredoc bodies in traversal (source) order, matching the order
    /// RuboCop's `each_node(:any_str)` collects them in, so `find_heredoc`
    /// picks the same one for a line covered by nested heredocs.
    heredocs: Vec<HeredocBody>,
}

impl TrailingWhitespace {
    /// RuboCop's `find_heredoc`: the first recorded heredoc whose body
    /// covers `line`.
    fn find_heredoc(&self, line: u32) -> Option<HeredocBody> {
        self.heredocs.iter().copied().find(|h| line >= h.first_line && line < h.end_line)
    }

    /// Records a heredoc's body when `node` is one, given its opening and
    /// closing delimiter spans.
    fn record(&mut self, ctx: &Context<'_>, opening: Span, closing: Span) {
        let open = ctx.text(opening);
        if !open.starts_with(b"<<") {
            return;
        }
        let first_line = ctx.line_col(opening.start).line + 1;
        let end_line = ctx.line_col(closing.start).line;
        let indent = if first_line < end_line {
            let body_start = ctx.line_span(first_line).start;
            indent_level(ctx.text(Span::new(body_start, closing.start)))
        } else {
            0
        };
        self.heredocs.push(HeredocBody {
            first_line,
            end_line,
            indent,
            is_static: open.ends_with(b"'"),
        });
    }
}

impl Rule for TrailingWhitespace {
    const META: RuleMeta = RuleMeta {
        name: "Layout/TrailingWhitespace",
        department: Department::Layout,
        summary: "Looks for trailing whitespace in the source code.",
        explanation: "\
Trailing whitespace at the end of a line is invisible noise that shows up in
diffs. The fix deletes it.

```ruby
# bad (the line ends with a space)
x = 0\x20

# good
x = 0
```

Inside a heredoc, whitespace that is part of the string cannot simply be
deleted, so the fix wraps it in an interpolation instead:

```ruby
# bad
code = <<~RUBY
  x = 0\x20
RUBY

# good
code = <<~RUBY
  x = 0#{'\x20'}
RUBY
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::StringNode,
            NodeKind::InterpolatedStringNode,
            NodeKind::XStringNode,
            NodeKind::InterpolatedXStringNode,
        ],
        config: &[ConfigOption {
            name: "AllowInHeredoc",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Allow trailing whitespace inside heredoc bodies.",
        }],
        blind_spots: "\
Lines are read from the file as-is, so a `\\r` before the line terminator is
treated as part of the line terminator rather than as trailing whitespace,
matching RuboCop's buffer handling.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_in_heredoc: options.bool("AllowInHeredoc"), heredocs: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.heredocs.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::StringNode { .. } => {
                let node = node.as_string_node().expect("kind matched");
                if let (Some(open), Some(close)) = (node.opening_loc(), node.closing_loc()) {
                    self.record(ctx, open.span(), close.span());
                }
            }
            Node::InterpolatedStringNode { .. } => {
                let node = node.as_interpolated_string_node().expect("kind matched");
                if let (Some(open), Some(close)) = (node.opening_loc(), node.closing_loc()) {
                    self.record(ctx, open.span(), close.span());
                }
            }
            Node::XStringNode { .. } => {
                let node = node.as_x_string_node().expect("kind matched");
                self.record(ctx, node.opening_loc().span(), node.closing_loc().span());
            }
            Node::InterpolatedXStringNode { .. } => {
                let node = node.as_interpolated_x_string_node().expect("kind matched");
                self.record(ctx, node.opening_loc().span(), node.closing_loc().span());
            }
            _ => {}
        }
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        // RuboCop inspects `processed_source.lines`, which stops at a
        // `__END__` data section.
        let last_line = match ctx.parsed().data_span() {
            Some(data) => ctx.line_col(data.start).line.saturating_sub(1),
            None => ctx.line_count(),
        };
        let source_len = u32::try_from(ctx.source().bytes().len()).unwrap_or(u32::MAX);

        for (line, span) in ctx.lines().take_while(|&(line, _)| line <= last_line) {
            let text = ctx.text(span);
            let Some(offset) = trailing_blank_start(text) else { continue };
            let heredoc = self.find_heredoc(line);
            if self.allow_in_heredoc && heredoc.is_some() {
                continue;
            }
            let range = Span::new(span.start + u32::try_from(offset).unwrap_or(0), span.end);
            match heredoc {
                None => ctx.report_with_fix(
                    &Self::META,
                    range,
                    MSG,
                    Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(range)] },
                ),
                Some(heredoc) => {
                    // RuboCop's `whitespace_only?` expands the offense over
                    // `[ \t]` and one newline on each side and checks it
                    // reached a newline both ways: true exactly when the
                    // offense starts the line and the line is terminated.
                    let whitespace_only = offset == 0 && span.end < source_len;
                    match heredoc_fix(ctx, range, whitespace_only, heredoc) {
                        Some(fix) => ctx.report_with_fix(&Self::META, range, MSG, fix),
                        None => ctx.report(&Self::META, range, MSG),
                    }
                }
            }
        }
    }
}

/// RuboCop's `process_line_in_heredoc`: whitespace that is only the body's
/// indentation is removed, anything else is wrapped in an interpolation
/// (impossible in a single-quoted heredoc, which is left uncorrected).
fn heredoc_fix(
    ctx: &Context<'_>,
    range: Span,
    whitespace_only: bool,
    heredoc: HeredocBody,
) -> Option<Fix> {
    if whitespace_only && char_len(ctx.text(range)) <= heredoc.indent {
        return Some(Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(range)] });
    }
    if heredoc.is_static {
        return None;
    }
    let start = if whitespace_only { range.start + heredoc.indent } else { range.start };
    Some(Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::insert(start, b"#{'".to_vec()), Edit::insert(range.end, b"'}".to_vec())],
    })
}

/// RuboCop's `Heredoc#indent_level`: the smallest leading-whitespace run of
/// the body's non-blank lines, or zero when every line is blank.
fn indent_level(body: &[u8]) -> u32 {
    let mut min: Option<u32> = None;
    for line in body.split_inclusive(|&b| b == b'\n') {
        let run = line.iter().take_while(|b| b.is_ascii_whitespace()).count();
        if run == line.len() && line.last() == Some(&b'\n') {
            // Blank line: its leading-whitespace run swallows the newline,
            // and RuboCop rejects those.
            continue;
        }
        let run = u32::try_from(run).unwrap_or(u32::MAX);
        min = Some(min.map_or(run, |m: u32| m.min(run)));
    }
    min.unwrap_or(0)
}

/// Byte offset of the trailing `[[:blank:]]` run of `line`, if it has one.
fn trailing_blank_start(line: &[u8]) -> Option<usize> {
    let mut end = line.len();
    while end > 0 {
        let last = line[end - 1];
        if last == b' ' || last == b'\t' {
            end -= 1;
            continue;
        }
        if last < 0x80 {
            break;
        }
        // Possible trailing multi-byte Unicode blank: walk back to this
        // character's lead byte and decode just that one character,
        // instead of validating the whole line as UTF-8.
        let mut start = end - 1;
        while start > 0 && (line[start] & 0xC0) == 0x80 {
            start -= 1;
        }
        match std::str::from_utf8(&line[start..end]).ok().and_then(|s| s.chars().next()) {
            Some(ch) if is_blank(ch) => end = start,
            _ => break,
        }
    }
    (end < line.len()).then_some(end)
}

/// Ruby's `[[:blank:]]`: horizontal tab plus every Unicode space separator.
fn is_blank(ch: char) -> bool {
    matches!(
        ch,
        '\t' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}
