//! `Layout/HeredocIndentation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/heredoc_indentation.rb` plus the `Heredoc` and
//! `Alignment` mixins it includes.
//!
//! # Locations
//!
//! Whitequark's `heredoc_body`/`heredoc_end` locations have no direct Prism
//! equivalent, so they are recomputed from the node's `opening`/`closing`
//! locations (which every heredoc-shaped `StringNode`/`InterpolatedStringNode`/
//! `XStringNode`/`InterpolatedXStringNode` carries): the body spans from the
//! start of the physical line right after the opener to the start of the
//! closing delimiter's physical line (Prism's `closing_loc` itself starts
//! there too, but -- unlike whitequark's `heredoc_end` -- also swallows the
//! delimiter's own leading whitespace and trailing newline, so `heredoc_end`
//! is rebuilt as that whole physical line instead, via [`Context::line_span`]).

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{each_descendant, ext, LocationExt as _, Node, NodeKind};
use ruby_source::Span;
use std::collections::HashMap;

/// The three ways a heredoc's opening delimiter can be spelled -- RuboCop's
/// `heredoc_indent_type`, which returns `'~'`, `'-'` or `nil`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndentType {
    /// `<<~`.
    Squiggly,
    /// `<<-`.
    Dash,
    /// Bare `<<`.
    Bare,
}

impl IndentType {
    /// Reads the type out of the opening delimiter's own source text
    /// (`node.source[/^<<([~-])/, 1]`); every heredoc opening starts with
    /// `<<`, so byte 2 (if any) is the only thing that can distinguish them.
    fn from_opening(opening: &[u8]) -> Self {
        match opening.get(2) {
            Some(b'~') => Self::Squiggly,
            Some(b'-') => Self::Dash,
            _ => Self::Bare,
        }
    }

    /// Byte length of the delimiter marker itself (`<<~`/`<<-`/`<<`), used to
    /// build the narrow edit that turns it into `<<~`.
    fn marker_len(self) -> u32 {
        match self {
            Self::Squiggly | Self::Dash => 3,
            Self::Bare => 2,
        }
    }

    /// `current_indent_type`: the marker text named in `TYPE_MSG`. Never
    /// read for `Squiggly` (its message is `WIDTH_MSG`, which names no type).
    fn current_type(self) -> &'static str {
        match self {
            Self::Squiggly => "<<~",
            Self::Dash => "<<-",
            Self::Bare => "<<",
        }
    }
}

/// Checks the indentation of the here document bodies. The bodies are
/// indented one step.
#[derive(Debug, Clone)]
pub struct HeredocIndentation {
    /// `Alignment#configured_indentation_width`: this cop's own
    /// (undocumented, but honoured) `IndentationWidth`, else
    /// `Layout/IndentationWidth`'s `Width`, else `2`.
    indentation_width: i64,
    /// `Layout/LineLength`'s configured `Max`, `None` when that cop is
    /// disabled (`max_line_length`'s `config.cop_enabled?` guard).
    max_line_length: Option<i64>,
    /// `Layout/LineLength`'s `AllowHeredoc` (default `true`).
    allow_heredoc_overflow: bool,
    /// Where each heredoc's body physically starts, keyed by the heredoc's
    /// own opening `Span::start`. Ruby's lexer fills stacked heredoc bodies
    /// (multiple `<<~TAG` openers on one physical line, e.g.
    /// `foo(<<~A, <<~B)`) in opener order, so a later opener's body does
    /// not begin right after its own line -- it begins right after the
    /// *previous pending* heredoc's closing delimiter line.
    /// `opening_loc`/the physical opener line never reflects that, but
    /// `closing_loc` (read straight off the node) does. This map is built
    /// once in [`Rule::file_start`] by collecting every heredoc in the
    /// file and sorting by opening offset -- true lexer order -- rather
    /// than relying on tree-visit order, which does not always match
    /// source order (e.g. a modifier-`if`'s predicate is visited before
    /// its statement, though it reads after it).
    body_start_lines: HashMap<u32, u32>,
}

impl HeredocIndentation {
    /// The heredoc's own `opening`/`closing` locations, for any
    /// heredoc-shaped `StringNode`/`InterpolatedStringNode`/`XStringNode`/
    /// `InterpolatedXStringNode`; `None` for anything else (or a heredoc
    /// missing a location, which cannot happen but Prism's API is
    /// `Option`-typed).
    pub(crate) fn heredoc_locs(node: &Node<'_>) -> Option<(Span, Span)> {
        if !ext::is_heredoc(node) {
            return None;
        }
        Some(match node {
            Node::StringNode { .. } => {
                let n = node.as_string_node().expect("kind matched");
                let (Some(o), Some(c)) = (n.opening_loc(), n.closing_loc()) else { return None };
                (o.span(), c.span())
            }
            Node::InterpolatedStringNode { .. } => {
                let n = node.as_interpolated_string_node().expect("kind matched");
                let (Some(o), Some(c)) = (n.opening_loc(), n.closing_loc()) else { return None };
                (o.span(), c.span())
            }
            Node::XStringNode { .. } => {
                let n = node.as_x_string_node().expect("kind matched");
                (n.opening_loc().span(), n.closing_loc().span())
            }
            Node::InterpolatedXStringNode { .. } => {
                let n = node.as_interpolated_x_string_node().expect("kind matched");
                (n.opening_loc().span(), n.closing_loc().span())
            }
            _ => return None,
        })
    }

    /// RuboCop's `on_heredoc`, given the heredoc's own `opening`/`closing`
    /// locations (every heredoc-shaped string/xstring node has both).
    fn check_heredoc(&self, ctx: &mut Context<'_>, opening: Span, closing: Span) {
        let indent_type = IndentType::from_opening(ctx.text(opening));

        let opening_line = ctx.line_col(opening.start).line;
        let body_start_line =
            self.body_start_lines.get(&opening.start).copied().unwrap_or(opening_line + 1);
        let body_span = Span::new(ctx.line_span(body_start_line).start, closing.start);
        let body = ctx.text(body_span);
        if body.iter().all(u8::is_ascii_whitespace) {
            return;
        }

        let body_indent_level = indent_level(body);
        let base_indent_level = indent_level(ctx.line_text(opening_line));

        match indent_type {
            IndentType::Squiggly => {
                let expected = base_indent_level + self.indentation_width;
                if expected == body_indent_level {
                    return;
                }
            }
            IndentType::Dash | IndentType::Bare => {
                if body_indent_level != 0 {
                    return;
                }
            }
        }

        if self.line_too_long(body, body_indent_level, base_indent_level) {
            return;
        }

        self.register_offense(
            ctx,
            opening,
            closing,
            body_span,
            body,
            body_indent_level,
            base_indent_level,
            indent_type,
        );
    }

    /// RuboCop's `line_too_long?`: when `Layout/LineLength` is active and
    /// does not itself exempt heredocs, whether the corrected body would
    /// overflow its configured `Max` -- in which case this cop stands down
    /// to avoid piling a `Layout/LineLength` offense onto every line.
    fn line_too_long(&self, body: &[u8], body_indent_level: i64, base_indent_level: i64) -> bool {
        let Some(max_line_length) = self.max_line_length else { return false };
        if self.allow_heredoc_overflow {
            return false;
        }
        let expected_indent = base_indent_level + self.indentation_width;
        let increase_indent_level = expected_indent - body_indent_level;
        longest_line_len(body) + increase_indent_level >= max_line_length
    }

    /// RuboCop's `register_offense` plus `message`/`type_message`/
    /// `width_message`/`adjust_squiggly`/`adjust_minus`.
    #[allow(clippy::too_many_arguments)]
    fn register_offense(
        &self,
        ctx: &mut Context<'_>,
        opening: Span,
        closing: Span,
        body_span: Span,
        body: &[u8],
        body_indent_level: i64,
        base_indent_level: i64,
        indent_type: IndentType,
    ) {
        let width = self.indentation_width;
        let message = match indent_type {
            IndentType::Squiggly => format!("Use {width} spaces for indentation in a heredoc."),
            IndentType::Dash | IndentType::Bare => format!(
                "Use {width} spaces for indentation in a heredoc by using `<<~` instead of `{}`.",
                indent_type.current_type()
            ),
        };

        let fix = match indent_type {
            IndentType::Squiggly => {
                let correct_indent_level = base_indent_level + self.indentation_width;
                let mut edits = vec![Edit::replace(
                    body_span,
                    build_indented_body(body, body_indent_level, correct_indent_level),
                )];

                let end_line = ctx.line_col(closing.start).line;
                let end_span = ctx.line_span(end_line);
                let end_text = ctx.text(end_span);
                let end_indent_level = indent_level(end_text);
                if end_indent_level < base_indent_level {
                    let spaces = usize::try_from(base_indent_level).unwrap_or(0);
                    let skip = usize::try_from(end_indent_level).unwrap_or(0);
                    let mut new_end = vec![b' '; spaces];
                    new_end.extend_from_slice(&end_text[skip..]);
                    edits.push(Edit::replace(end_span, new_end));
                }
                Fix { applicability: Applicability::Safe, edits }
            }
            IndentType::Dash | IndentType::Bare => {
                let marker = Span::new(opening.start, opening.start + indent_type.marker_len());
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(marker, b"<<~".as_slice())],
                }
            }
        };

        ctx.report_with_fix(&Self::META, body_span, message, fix);
    }
}

/// RuboCop's `Heredoc#indent_level`: the smallest leading-whitespace run
/// among a text's non-blank lines (`0` if every line is blank, or the text
/// is empty). A line whose own line terminator is included in its leading
/// run is blank and skipped -- except the text's final, unterminated line
/// (if any), which is never skipped, matching `str.lines.reject { |line|
/// line.end_with?("\n") }`'s inability to reject an entry that has no `"\n"`
/// to end with. Used both for a heredoc body (always `"\n"`-terminated,
/// where this matters) and for a single physical line with no terminator at
/// all (a heredoc's own opening line, or its closing delimiter line, where
/// every line is "final" and so never skipped).
fn indent_level(text: &[u8]) -> i64 {
    let len = text.len();
    let mut start = 0;
    let mut min: Option<i64> = None;
    while start < len {
        let newline = text[start..].iter().position(|&b| b == b'\n');
        let (line, next, terminated) = match newline {
            Some(pos) => (&text[start..start + pos], start + pos + 1, true),
            None => (&text[start..], len, false),
        };
        let lead = line.iter().take_while(|&&b| b == b' ' || b == b'\t').count();
        let blank = lead == line.len();
        if !(blank && terminated) {
            let lead = i64::try_from(lead).unwrap_or(i64::MAX);
            min = Some(min.map_or(lead, |m| m.min(lead)));
        }
        start = next;
    }
    min.unwrap_or(0)
}

/// RuboCop's `longest_line`'s `.size` (chomped): the longest physical line
/// in `text`, trailing `"\r\n"`/`"\n"` excluded.
fn longest_line_len(text: &[u8]) -> i64 {
    let len = text.len();
    let mut start = 0;
    let mut max = 0i64;
    loop {
        let newline = text[start..].iter().position(|&b| b == b'\n');
        let mut end = match newline {
            Some(pos) => start + pos,
            None => len,
        };
        if end > start && text[end - 1] == b'\r' {
            end -= 1;
        }
        max = max.max(i64::try_from(end - start).unwrap_or(i64::MAX));
        match newline {
            Some(pos) => start = start + pos + 1,
            None => break,
        }
        if start >= len {
            break;
        }
    }
    max
}

/// RuboCop's `indented_body`: replaces exactly `body_indent_level` leading
/// non-newline whitespace characters at the start of every physical line
/// with `correct_indent_level` spaces. A line with fewer than
/// `body_indent_level` such characters (a blank line shorter than the old
/// indentation, most commonly) has no match and is left untouched.
fn build_indented_body(body: &[u8], body_indent_level: i64, correct_indent_level: i64) -> Vec<u8> {
    let body_indent_level = usize::try_from(body_indent_level).unwrap_or(0);
    let correct_indent_level = usize::try_from(correct_indent_level).unwrap_or(0);
    let len = body.len();
    let mut out = Vec::with_capacity(len + len / 4 + correct_indent_level);
    let mut start = 0;
    loop {
        let newline = body[start..].iter().position(|&b| b == b'\n');
        let line_end = match newline {
            Some(pos) => start + pos,
            None => len,
        };
        let line = &body[start..line_end];
        let lead = line.iter().take_while(|&&b| b == b' ' || b == b'\t').count();
        if lead >= body_indent_level {
            out.resize(out.len() + correct_indent_level, b' ');
            out.extend_from_slice(&line[body_indent_level..]);
        } else {
            out.extend_from_slice(line);
        }
        match newline {
            Some(pos) => {
                out.push(b'\n');
                start = start + pos + 1;
            }
            None => break,
        }
        if start >= len {
            break;
        }
    }
    out
}

impl Rule for HeredocIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/HeredocIndentation",
        department: Department::Layout,
        summary: "Checks the indentation of the here document bodies.",
        explanation: "\
Checks the indentation of the here document bodies. The bodies are indented
one step.

NOTE: When `Layout/LineLength`'s `AllowHeredoc` is `false` (not default),
this cop does not add any offenses for long here documents to avoid
`Layout/LineLength`'s offenses.

```ruby
# bad
<<-RUBY
something
RUBY

# good
<<~RUBY
  something
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
            name: "IndentationWidth",
            default: ConfigDefault::Nil,
            allowed: &[],
            doc: "Overrides `Layout/IndentationWidth`'s configured width for this cop alone; \
                  used both to check the body's indentation and, during autocorrection, to \
                  determine how many spaces should replace each tab.",
        }],
        blind_spots: "\
RuboCop's `minimum_target_ruby_version 2.3` guard is not ported: the scenario it exists for \
(`RSpec` `:ruby22`) is itself marked `unsupported_on: :prism` upstream, since Prism -- the only \
parser this cop is ported against -- never targets a Ruby that old.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let indentation_width = options
            .get("IndentationWidth")
            .and_then(OptionValue::as_int)
            .or_else(|| {
                options.peer("Layout/IndentationWidth", "Width").and_then(OptionValue::as_int)
            })
            .unwrap_or(2);
        let max_line_length = {
            let enabled = options
                .peer("Layout/LineLength", "Enabled")
                .and_then(OptionValue::as_bool)
                .unwrap_or(true);
            enabled.then(|| {
                options
                    .peer("Layout/LineLength", "Max")
                    .and_then(OptionValue::as_int)
                    .unwrap_or(120)
            })
        };
        let allow_heredoc_overflow = options
            .peer("Layout/LineLength", "AllowHeredoc")
            .and_then(OptionValue::as_bool)
            .unwrap_or(true);
        Ok(Self {
            indentation_width,
            max_line_length,
            allow_heredoc_overflow,
            body_start_lines: HashMap::new(),
        })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let mut heredocs: Vec<(Span, Span)> = Vec::new();
        each_descendant(&ctx.parsed().root(), &mut |n| {
            if let Some(locs) = Self::heredoc_locs(n) {
                heredocs.push(locs);
            }
        });
        // Lexer order is opening-offset order, not tree-visit order (e.g. a
        // modifier-`if`'s predicate is visited before its statement, though
        // it reads after it).
        heredocs.sort_by_key(|(opening, _)| opening.start);

        let mut cursor = 0u32;
        self.body_start_lines.clear();
        for (opening, closing) in heredocs {
            let opening_line = ctx.line_col(opening.start).line;
            let closing_line = ctx.line_col(closing.start).line;
            let body_start_line = (opening_line + 1).max(cursor);
            cursor = cursor.max(closing_line + 1);
            self.body_start_lines.insert(opening.start, body_start_line);
        }
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some((opening, closing)) = Self::heredoc_locs(node) else { return };
        self.check_heredoc(ctx, opening, closing);
    }
}

#[cfg(test)]
mod tests {
    use super::{build_indented_body, indent_level, longest_line_len};

    #[test]
    fn indent_level_ignores_blank_terminated_lines_but_not_the_final_untermianted_one() {
        assert_eq!(indent_level(b"  foo\n\n    bar\n"), 2);
        assert_eq!(indent_level(b"  foo\n    \n    bar\n"), 2);
        assert_eq!(indent_level(b""), 0);
        assert_eq!(indent_level(b"    "), 4);
        assert_eq!(indent_level(b"  MSG"), 2);
    }

    #[test]
    fn longest_line_len_chomps_each_line() {
        assert_eq!(longest_line_len(b"ab\nabcdef\nabc\n"), 6);
        assert_eq!(longest_line_len(b"ab\r\nabcdef\r\n"), 6);
    }

    #[test]
    fn build_indented_body_skips_lines_shorter_than_the_old_indent() {
        let body = b"              foo\n\n                bar\n";
        let out = build_indented_body(body, 14, 16);
        assert_eq!(out, b"                foo\n\n                  bar\n".to_vec());

        let body = b"              foo\n    \n                bar\n";
        let out = build_indented_body(body, 14, 16);
        assert_eq!(out, b"                foo\n    \n                  bar\n".to_vec());
    }
}
