//! `Style/RegexpLiteral`, ported from RuboCop's
//! `lib/rubocop/cop/style/regexp_literal.rb`.
//!
//! Prism gives every regexp literal a [`NodeKind::RegularExpressionNode`]
//! (no interpolation) or [`NodeKind::InterpolatedRegularExpressionNode`]
//! (has interpolation); both map onto upstream's single `on_regexp` entry
//! point. Upstream's `node_body` (raw source of the literal's `str`-typed
//! children only, interpolated code excluded) becomes, here, the raw source
//! spans of the `StringNode` parts of an interpolated literal (or the whole
//! `content_loc` of a plain one); `node_body(include_begin_nodes: true)`
//! (used only for locating inner slashes to swap during autocorrection)
//! additionally includes each `EmbeddedStatementsNode`'s own full span
//! (`#{...}` delimiters included, matching whitequark's `begin`-typed
//! interpolation child, whose `.source` also spans the whole `#{...}`).
//!
//! `Style/PercentLiteralDelimiters`'s `PreferredDelimiters['%r']` and
//! `Style/MethodCallWithArgsParentheses`'s `EnforcedStyle` are both read as
//! peer cop settings (`RuleOptions::peer`), exactly like RuboCop's
//! `config.for_cop(...)`.
//!
//! `node.parent&.call_type?` (deciding whether a `%r/.../` literal is a bare
//! call argument, for the "would create invalid syntax" exception) is
//! bridged across Prism's `ArgumentsNode` wrapper -- absent from whitequark,
//! where a call's arguments are its own direct children -- by skipping
//! exactly one such wrapper.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_USE_SLASHES: &str = "Use `//` around regular expression.";
const MSG_USE_PERCENT_R: &str = "Use `%r` around regular expression.";

/// RuboCop's `ConfigurableEnforcedStyle` `style` for this cop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// Always use slashes.
    Slashes,
    /// Always use `%r`.
    PercentR,
    /// Slashes on single-line regexes, `%r` on multi-line ones.
    Mixed,
}

/// Use / or %r around regular expressions.
#[derive(Debug, Clone)]
pub struct RegexpLiteral {
    style: Style,
    allow_inner_slashes: bool,
    /// `%r`'s preferred delimiter pair, from `Style/PercentLiteralDelimiters`'s
    /// `PreferredDelimiters` (its own `%r` entry, else its `default` entry,
    /// else `{}`, matching `config/default.yml`'s own default for `%r`).
    delimiters: (u8, u8),
    /// `delimiters`, restricted to the four paired delimiter kinds upstream's
    /// `PAIR_DELIMITER_PATTERNS` recognizes (`()`, `[]`, `{}`, `<>`): when
    /// present, a `/.../ ` literal whose body has unbalanced occurrences of
    /// these chars cannot safely become `%r<open>...<close>` and is skipped
    /// entirely.
    paired_delimiters: Option<(u8, u8)>,
    /// Whether `Style/MethodCallWithArgsParentheses`'s `EnforcedStyle` is
    /// `omit_parentheses` (else RuboCop's own default, `require_parentheses`).
    omit_parentheses_style: bool,
}

impl Rule for RegexpLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Style/RegexpLiteral",
        department: Department::Style,
        summary: "Use / or %r around regular expressions.",
        explanation: "Enforces using `//` or `%r` around regular expressions.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::RegularExpressionNode, NodeKind::InterpolatedRegularExpressionNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("slashes"),
                allowed: &["slashes", "percent_r", "mixed"],
                doc: "The preferred style for regular expression literals.",
            },
            ConfigOption {
                name: "AllowInnerSlashes",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether an inner unescaped slash is allowed to stay in a slash literal \
                      instead of forcing `%r`.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "percent_r" => Style::PercentR,
            "mixed" => Style::Mixed,
            _ => Style::Slashes,
        };
        let allow_inner_slashes = options.bool("AllowInnerSlashes");
        let delimiters = preferred_delimiters(options);
        let paired_delimiters = match delimiters {
            (b'(', b')') | (b'[', b']') | (b'{', b'}') | (b'<', b'>') => Some(delimiters),
            _ => None,
        };
        let omit_parentheses_style = options
            .peer("Style/MethodCallWithArgsParentheses", "EnforcedStyle")
            .and_then(OptionValue::as_str)
            .unwrap_or("require_parentheses")
            == "omit_parentheses";
        Ok(Self {
            style,
            allow_inner_slashes,
            delimiters,
            paired_delimiters,
            omit_parentheses_style,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(RegexpParts {
            opening_span,
            closing_span,
            str_fragments,
            autocorrect_fragments,
            starts_with_blank_or_eq,
        }) = regexp_parts(node)
        else {
            return;
        };
        let node_span = node.span();

        let opening_text = ctx.text(opening_span);
        let slash_literal = opening_text == b"/";

        // RuboCop's `return if slash_literal?(node) && percent_r_delimiters_conflict?(node)`:
        // converting to `%r<open>...<close>` would misparse if the body has
        // unbalanced occurrences of that pair's characters.
        if slash_literal {
            if let Some((open, close)) = self.paired_delimiters {
                if !balanced_delimiters(ctx, &str_fragments, open, close) {
                    return;
                }
            }
        }

        let contains_slash = str_fragments.iter().any(|s| ctx.text(*s).contains(&b'/'));
        let contains_disallowed_slash = !self.allow_inner_slashes && contains_slash;
        let single_line = ctx.is_single_line(node_span);
        let multiline = !single_line;

        let message = if slash_literal {
            let allowed_mixed_slash =
                self.style == Style::Mixed && single_line && !contains_disallowed_slash;
            let allowed =
                (self.style == Style::Slashes && !contains_disallowed_slash) || allowed_mixed_slash;
            if allowed {
                None
            } else {
                Some(MSG_USE_PERCENT_R)
            }
        } else {
            let allowed_omit_parens = logical_parent_is_call(ctx)
                && (starts_with_blank_or_eq || self.omit_parentheses_style);
            let allowed_mixed_percent_r =
                (self.style == Style::Mixed && multiline) || contains_disallowed_slash;
            let allowed = (self.style == Style::Slashes && contains_disallowed_slash)
                || self.style == Style::PercentR
                || allowed_mixed_percent_r
                || allowed_omit_parens;
            if allowed {
                None
            } else {
                Some(MSG_USE_SLASHES)
            }
        };
        let Some(message) = message else { return };

        // `correct_delimiters`: swap the opening token and just the closing
        // delimiter byte, leaving any trailing regopt flags (`x`, `i`, ...)
        // -- folded into Prism's `closing_loc`, unlike whitequark's
        // flags-excluded `loc.end` -- untouched.
        let new_opening: Vec<u8> =
            if slash_literal { vec![b'%', b'r', self.delimiters.0] } else { vec![b'/'] };
        let new_closing_delim = if slash_literal { self.delimiters.1 } else { b'/' };
        let closing_delim_span = Span::new(closing_span.start, closing_span.start + 1);

        // `correct_inner_slashes`: swap every occurrence of the delimiter's
        // "inner slash" spelling (escaped `\/` when either the current or
        // the new delimiter is itself a slash, else bare `/`) across the
        // literal's raw body (including interpolated statements' source, to
        // match `node_body(include_begin_nodes: true)`).
        let before_pattern = inner_slash_for(opening_text);
        let after_pattern = inner_slash_for(&new_opening);

        let mut edits = Vec::with_capacity(2);
        edits.push(Edit::replace(opening_span, new_opening));
        edits.push(Edit::replace(closing_delim_span, vec![new_closing_delim]));

        let body: Vec<u8> =
            autocorrect_fragments.iter().flat_map(|s| ctx.text(*s).iter().copied()).collect();
        let regexp_begin = opening_span.end;
        for idx in find_all_overlapping(&body, before_pattern) {
            let start = regexp_begin + u32::try_from(idx).unwrap_or(u32::MAX);
            let span =
                Span::new(start, start + u32::try_from(before_pattern.len()).unwrap_or(u32::MAX));
            edits.push(Edit::replace(span, after_pattern.to_vec()));
        }

        let fix = Fix { applicability: Applicability::Safe, edits };
        ctx.report_with_fix(&Self::META, node_span, message, fix);
    }
}

/// The result of destructuring a regexp node (plain or interpolated).
struct RegexpParts {
    opening_span: Span,
    closing_span: Span,
    /// The spans of the literal `str`-typed body (used for offense detection).
    str_fragments: Vec<Span>,
    /// The spans forming the raw source body including interpolated
    /// `#{...}` blocks (used for autocorrection).
    autocorrect_fragments: Vec<Span>,
    /// Whether the unescaped content starts with a space or `=` (RuboCop's
    /// `include_or_starts_with_specific_chars?`).
    starts_with_blank_or_eq: bool,
}

/// Destructures a regexp node (plain or interpolated) into its
/// [`RegexpParts`]. Returns `None` for any other node kind.
fn regexp_parts(node: &Node<'_>) -> Option<RegexpParts> {
    match node {
        Node::RegularExpressionNode { .. } => {
            let n = node.as_regular_expression_node().expect("kind matched");
            let content_span = n.content_loc().span();
            let starts = matches!(n.unescaped().first(), Some(b' ' | b'='));
            Some(RegexpParts {
                opening_span: n.opening_loc().span(),
                closing_span: n.closing_loc().span(),
                str_fragments: vec![content_span],
                autocorrect_fragments: vec![content_span],
                starts_with_blank_or_eq: starts,
            })
        }
        Node::InterpolatedRegularExpressionNode { .. } => {
            let n = node.as_interpolated_regular_expression_node().expect("kind matched");
            let mut str_fragments = Vec::new();
            let mut autocorrect_fragments = Vec::new();
            let mut starts_with_blank_or_eq = false;
            let mut found_start = false;
            for part in &n.parts() {
                if let Some(s) = part.as_string_node() {
                    let span = s.content_loc().span();
                    str_fragments.push(span);
                    autocorrect_fragments.push(span);
                    if !found_start {
                        if let Some(&b) = s.unescaped().first() {
                            starts_with_blank_or_eq = b == b' ' || b == b'=';
                            found_start = true;
                        }
                    }
                } else if part.as_embedded_statements_node().is_some() {
                    // Whitequark's `begin`-typed interpolation child's `.source`
                    // spans the whole `#{...}` (delimiters included, unlike its
                    // inner `statements`), so `node_body(include_begin_nodes:
                    // true)` reconstructs the literal's raw text with no gaps;
                    // mirrored here with the part's own full span.
                    autocorrect_fragments.push(part.span());
                }
            }
            Some(RegexpParts {
                opening_span: n.opening_loc().span(),
                closing_span: n.closing_loc().span(),
                str_fragments,
                autocorrect_fragments,
                starts_with_blank_or_eq,
            })
        }
        _ => None,
    }
}

/// `Style/PercentLiteralDelimiters`'s `PreferredDelimiters` option, read as
/// a peer cop setting: its own `%r` entry, else its `default` entry, else
/// `{}` (matching `config/default.yml`'s own `%r`-specific default).
fn preferred_delimiters(options: &RuleOptions) -> (u8, u8) {
    let map = options.peer("Style/PercentLiteralDelimiters", "PreferredDelimiters");
    let entries = map.and_then(OptionValue::as_map).unwrap_or(&[]);
    let lookup =
        |key: &str| entries.iter().find(|(name, _)| name == key).and_then(|(_, v)| v.as_str());
    let delimiter = lookup("%r").or_else(|| lookup("default")).unwrap_or("{}");
    let mut bytes = delimiter.bytes();
    let open = bytes.next().unwrap_or(b'{');
    let close = bytes.next().unwrap_or(open);
    (open, close)
}

/// RuboCop's `balanced_delimiters?`: scans `fragments`' raw source (an
/// escaped char -- any byte following a `\` -- never counts as a delimiter),
/// tracking depth; unbalanced (negative mid-scan, or nonzero at the end)
/// fails.
fn balanced_delimiters(ctx: &Context<'_>, fragments: &[Span], open: u8, close: u8) -> bool {
    let mut depth: i32 = 0;
    let mut escaped = false;
    for span in fragments {
        for &b in ctx.text(*span) {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == open {
                depth += 1;
            } else if b == close {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
        }
    }
    depth == 0
}

/// RuboCop's `inner_slash_for`: an opening delimiter that is itself a slash
/// (`/` or `%r/`) requires inner slashes to already be escaped as `\/`, else
/// they must be bare `/`.
fn inner_slash_for(opening: &[u8]) -> &'static [u8] {
    if opening == b"/" || opening == b"%r/" {
        b"\\/"
    } else {
        b"/"
    }
}

/// Every (possibly overlapping) occurrence of `needle` in `haystack`,
/// matching Ruby's `text.index(pattern, index + 1)` loop (which restarts
/// just past each match's start, not its end).
fn find_all_overlapping(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    if needle.is_empty() || haystack.len() < needle.len() {
        return out;
    }
    for i in 0..=(haystack.len() - needle.len()) {
        if &haystack[i..i + needle.len()] == needle {
            out.push(i);
        }
    }
    out
}

/// RuboCop's `node.parent&.call_type?`, bridged across Prism's
/// `ArgumentsNode` wrapper (absent from whitequark, where a call's
/// arguments are its own direct children): true when the node one level up
/// from that wrapper (or the immediate parent, if there is no wrapper) is a
/// `CallNode`.
fn logical_parent_is_call(ctx: &Context<'_>) -> bool {
    let ancestors = ctx.ancestors();
    let Some(last_idx) = ancestors.len().checked_sub(1) else { return false };
    let idx = if ancestors[last_idx].kind == NodeKind::ArgumentsNode {
        match last_idx.checked_sub(1) {
            Some(i) => i,
            None => return false,
        }
    } else {
        last_idx
    };
    ancestors[idx].kind == NodeKind::CallNode
}
