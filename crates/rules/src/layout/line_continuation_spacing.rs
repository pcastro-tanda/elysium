//! `Layout/LineContinuationSpacing`, ported from RuboCop's
//! `lib/rubocop/cop/layout/line_continuation_spacing.rb`.
//!
//! Upstream scans `processed_source.raw_source.lines` and builds an
//! `ignored_ranges` set from the AST (string/`dstr`/percent-array literal
//! bodies, heredoc bodies, and comments) to skip backslashes that are not
//! real line continuations. Elysium's [`Context::opaque_spans`] already
//! covers string/x-string/regexp content and comments (what a raw-byte scan
//! must not read as code), so this port only adds its own tracking for a
//! percent-literal array (`%w[...]`, `%i(...)`, ...), which upstream treats
//! as fully opaque but `opaque_spans` does not cover (its elements are
//! themselves `str`/`symbol` nodes, but the *construct* needs to be ignored
//! wholesale, including the whitespace between elements). The `__END__`
//! data section is also skipped, matching upstream's lexer never tokenizing
//! past it.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{is_ruby_whitespace, Span};

/// RuboCop's `space_style?` message.
const MSG_SPACE: &str = "Use one space in front of backslash.";
/// RuboCop's `no_space_style?` message.
const MSG_NO_SPACE: &str = "Use zero spaces in front of backslash.";

/// Checks that the backslash of a line continuation is separated from
/// preceding text by exactly one space (default) or zero spaces.
#[derive(Debug, Clone)]
pub struct LineContinuationSpacing {
    /// `true` for `EnforcedStyle: no_space`, `false` for `space` (default).
    no_space: bool,
    /// Whole-node spans of percent-literal arrays (`%w`, `%i`, `%W`, `%I`),
    /// collected during traversal; upstream's `percent_literal?` check on
    /// `array_type?` nodes.
    percent_array_spans: Vec<Span>,
    /// Spans upstream's `ignored_literal_ranges` marks wholesale opaque for
    /// `str`/`dstr` nodes: a heredoc's body (`loc.heredoc_body`, excluding
    /// the opening/closing delimiter lines) or, for a quoted (non-heredoc)
    /// string literal, its whole `loc.expression` including any
    /// interpolated code -- unlike [`Context::opaque_spans`], which
    /// deliberately carves interpolated code back out for other cops.
    literal_spans: Vec<Span>,
}

impl Rule for LineContinuationSpacing {
    const META: RuleMeta = RuleMeta {
        name: "Layout/LineContinuationSpacing",
        department: Department::Layout,
        summary: "Checks the spacing in front of backslash in line continuations.",
        explanation: "\
```ruby
# EnforcedStyle: space (default)

# bad
'a'\\
'b'  \\
'c'

# good
'a' \\
'b' \\
'c'
```

```ruby
# EnforcedStyle: no_space

# bad
'a' \\
'b'  \\
'c'

# good
'a'\\
'b'\\
'c'
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ArrayNode, NodeKind::StringNode, NodeKind::InterpolatedStringNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("space"),
            allowed: &["space", "no_space"],
            doc: "The spacing style to enforce in front of a line-continuation backslash.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let no_space = options.style("EnforcedStyle")? == "no_space";
        Ok(Self { no_space, percent_array_spans: Vec::new(), literal_spans: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(array) = node.as_array_node() {
            let Some(opening) = array.opening_loc() else { return };
            if ctx.text(opening.span()).starts_with(b"%") {
                self.percent_array_spans.push(node.span());
            }
            return;
        }
        if let Some(s) = node.as_string_node() {
            let Some(opening) = s.opening_loc() else { return };
            if ctx.text(opening.span()).starts_with(b"<<") {
                if let Some(closing) = s.closing_loc() {
                    self.literal_spans
                        .push(Span::new(s.content_loc().span().start, closing.span().start));
                }
            } else {
                self.literal_spans.push(node.span());
            }
            return;
        }
        if let Some(s) = node.as_interpolated_string_node() {
            let Some(opening) = s.opening_loc() else { return };
            if ctx.text(opening.span()).starts_with(b"<<") {
                if let Some(closing) = s.closing_loc() {
                    let body_start =
                        s.parts().iter().next().map_or(opening.span().end, |p| p.span().start);
                    self.literal_spans.push(Span::new(body_start, closing.span().start));
                }
            } else {
                self.literal_spans.push(node.span());
            }
        }
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        if !ctx.source().bytes().contains(&b'\\') {
            return;
        }
        let data_start = ctx.parsed().data_span().map(|s| s.start);

        for (_line, span) in ctx.lines() {
            if let Some(data_start) = data_start {
                if span.start >= data_start {
                    break;
                }
            }
            let bytes = ctx.text(span);
            if bytes.last() != Some(&b'\\') {
                continue;
            }
            let idx_backslash = bytes.len() - 1;
            let mut ws_count = 0usize;
            while ws_count < idx_backslash
                && is_ruby_whitespace(bytes[idx_backslash - 1 - ws_count])
            {
                ws_count += 1;
            }

            let (offense, correction): (bool, &[u8]) = if self.no_space {
                (ws_count >= 1, b"\\")
            } else {
                (ws_count == 0 || ws_count >= 2, b" \\")
            };
            if !offense {
                continue;
            }

            let start = span.start + u32::try_from(idx_backslash - ws_count).expect("fits u32");
            let end = span.start + u32::try_from(idx_backslash + 1).expect("fits u32");
            let range = Span::new(start, end);

            if ctx.in_opaque_span(start)
                || self
                    .percent_array_spans
                    .iter()
                    .chain(self.literal_spans.iter())
                    .any(|s| s.start <= range.start && range.end <= s.end)
            {
                continue;
            }

            let message = if self.no_space { MSG_NO_SPACE } else { MSG_SPACE };
            ctx.report_with_fix(
                &Self::META,
                range,
                message,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(range, correction.to_vec())],
                },
            );
        }
    }
}
