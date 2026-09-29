//! `Style/PercentLiteralDelimiters`, ported from RuboCop's
//! `lib/rubocop/cop/style/percent_literal_delimiters.rb`, plus the
//! `PercentLiteral` and `PreferredDelimiters` mixins it uses.
//!
//! Upstream's `type(node)` is `node.loc.begin.source[0..-2]` -- the opening
//! delimiter's source text minus its trailing bracket character, e.g. `"%i"`
//! for `%i[...]` or `"%"` for a bare `%(...)`. Only a percent-prefixed
//! opening ever produces one of the ten recognized types
//! (`PreferredDelimiters::PERCENT_LITERAL_TYPES`), so deriving it the same
//! way from each Prism node's `opening_loc` reproduces `percent_literal?`
//! (`begin_source.start_with?('%')`) and the per-hook type filter (`on_str`
//! only cares about `%`/`%q`/`%Q`, etc.) simultaneously: a plain `"string"`
//! or `[array]` has an opening that doesn't start with `%` and is filtered
//! out before its (nonsensical) "type" is ever looked at.
//!
//! `string_source`, called on `node.children` in upstream, behaves
//! differently depending on what those children actually are:
//!
//! - For a *bare* literal (`%(...)`, `%q(...)`, `%s(...)`, `%x(...)`) the
//!   node's own child is a raw Ruby `String`/`Symbol` *value*, so
//!   `string_source` takes the `.scrub`/`.to_s` branch -- the *decoded*
//!   content. This maps to Prism's `StringNode`/`SymbolNode`/`XStringNode`
//!   `unescaped()`.
//! - For a `%r(...)` regexp, the content is itself a nested `str`-type AST
//!   node (whitequark always wraps regexp content that way, interpolated or
//!   not), so `string_source` takes the `node.respond_to?(:type)` branch --
//!   the node's raw *source* text, escapes intact. This maps to
//!   `RegularExpressionNode`/its `InterpolatedRegularExpressionNode` string
//!   parts' `content_loc` source text.
//! - For a percent array (`%w`/`%W`/`%i`/`%I`), each element is itself a
//!   `str`/`sym` (or, if the word contains interpolation, `dstr`/`dsym`) AST
//!   node, so non-interpolated elements take the same raw-source branch as
//!   regexp content, and interpolated elements are filtered out entirely
//!   (never descended into).
//! - For an interpolated literal (`%Q`/`%W`/`%I`/`%r`/`%x` with real
//!   interpolation), the parts are a mix of literal `str`/`sym` AST-node
//!   fragments (raw source, same as above) and non-string interpolation
//!   nodes (filtered out).
//!
//! So only the single bare-literal case decodes escapes; every other shape
//! (array elements, interpolated parts, regexp content) compares raw source
//! bytes -- which is why `%w(\(some words\))`'s escaped `\(`/`\)` still
//! "contains" `(`/`)` for `include_same_character_as_used_for_delimiter?`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Location, LocationExt as _, Node, NodeExt as _, NodeKind};

/// `PreferredDelimiters::PERCENT_LITERAL_TYPES`, in the same order.
const PERCENT_LITERAL_TYPES: [&str; 10] =
    ["%", "%i", "%I", "%q", "%Q", "%r", "%s", "%w", "%W", "%x"];

/// `config/default.yml`'s `PreferredDelimiters`: types not listed here fall
/// back to `default: '()'`.
const DEFAULT_OVERRIDES: [(&str, &str); 5] =
    [("%i", "[]"), ("%I", "[]"), ("%r", "{}"), ("%w", "[]"), ("%W", "[]")];

/// `config/default.yml`'s `PreferredDelimiters/default`.
const DEFAULT_FALLBACK: &str = "()";

/// Use `%`-literal delimiters consistently.
#[derive(Debug, Clone)]
pub struct PercentLiteralDelimiters {
    /// Preferred `(opening, closing)` delimiter bytes for each of the ten
    /// [`PERCENT_LITERAL_TYPES`], in that same order.
    preferred: [(u8, u8); 10],
}

impl PercentLiteralDelimiters {
    /// The preferred `(opening, closing)` delimiter for `type` (one of
    /// [`PERCENT_LITERAL_TYPES`]), or `None` if `type` isn't recognized.
    fn preferred_for(&self, kind: &str) -> Option<(u8, u8)> {
        PERCENT_LITERAL_TYPES.iter().position(|t| *t == kind).map(|i| self.preferred[i])
    }
}

/// `PercentLiteral#process`: the node's own opening delimiter must start
/// with `%`; its type (the opening minus the trailing bracket character)
/// derives which literal kind it is. Returns `None` for a node with the
/// wrong kind, or with a location Prism couldn't recover.
fn literal_parts<'pr>(
    node: &Node<'pr>,
) -> Option<(Location<'pr>, Location<'pr>, Vec<Contents<'pr>>)> {
    Some(match node {
        Node::ArrayNode { .. } => {
            let array = node.as_array_node().expect("kind matched");
            let (Some(opening), Some(closing)) = (array.opening_loc(), array.closing_loc()) else {
                return None;
            };
            let contents: Vec<Contents<'pr>> = array
                .elements()
                .iter()
                .filter_map(|el| match &el {
                    Node::StringNode { .. } | Node::SymbolNode { .. } => {
                        Some(Contents::Raw(el.span()))
                    }
                    _ => None,
                })
                .collect();
            (opening, closing, contents)
        }
        Node::RegularExpressionNode { .. } => {
            let regexp = node.as_regular_expression_node().expect("kind matched");
            (
                regexp.opening_loc(),
                regexp.closing_loc(),
                vec![Contents::Raw(regexp.content_loc().span())],
            )
        }
        Node::InterpolatedRegularExpressionNode { .. } => {
            let regexp = node.as_interpolated_regular_expression_node().expect("kind matched");
            let contents = string_parts_raw(&regexp.parts().iter().collect::<Vec<_>>());
            (regexp.opening_loc(), regexp.closing_loc(), contents)
        }
        Node::StringNode { .. } => {
            let string = node.as_string_node().expect("kind matched");
            let (Some(opening), Some(closing)) = (string.opening_loc(), string.closing_loc())
            else {
                return None;
            };
            (opening, closing, vec![Contents::UnescapedString(string)])
        }
        Node::InterpolatedStringNode { .. } => {
            let string = node.as_interpolated_string_node().expect("kind matched");
            let (Some(opening), Some(closing)) = (string.opening_loc(), string.closing_loc())
            else {
                return None;
            };
            let contents = string_parts_raw(&string.parts().iter().collect::<Vec<_>>());
            (opening, closing, contents)
        }
        Node::SymbolNode { .. } => {
            let symbol = node.as_symbol_node().expect("kind matched");
            let (Some(opening), Some(closing)) = (symbol.opening_loc(), symbol.closing_loc())
            else {
                return None;
            };
            (opening, closing, vec![Contents::UnescapedSymbol(symbol)])
        }
        Node::InterpolatedSymbolNode { .. } => {
            let symbol = node.as_interpolated_symbol_node().expect("kind matched");
            let (Some(opening), Some(closing)) = (symbol.opening_loc(), symbol.closing_loc())
            else {
                return None;
            };
            let contents = string_parts_raw(&symbol.parts().iter().collect::<Vec<_>>());
            (opening, closing, contents)
        }
        Node::XStringNode { .. } => {
            let xstring = node.as_x_string_node().expect("kind matched");
            (
                xstring.opening_loc(),
                xstring.closing_loc(),
                vec![Contents::UnescapedXString(xstring)],
            )
        }
        Node::InterpolatedXStringNode { .. } => {
            let xstring = node.as_interpolated_x_string_node().expect("kind matched");
            let opening = xstring.opening_loc();
            let closing = xstring.closing_loc();
            let contents = string_parts_raw(&xstring.parts().iter().collect::<Vec<_>>());
            (opening, closing, contents)
        }
        _ => return None,
    })
}

impl Rule for PercentLiteralDelimiters {
    const META: RuleMeta = RuleMeta {
        name: "Style/PercentLiteralDelimiters",
        department: Department::Style,
        summary: "Use `%`-literal delimiters consistently.",
        explanation: "Enforces the consistent usage of `%`-literal delimiters.\n\nSpecify the \
                      `default` key to set all preferred delimiters at once. You can continue \
                      to specify individual preferred delimiters to override the default.\n\n\
                      ```ruby\n# Style/PercentLiteralDelimiters:\n#   PreferredDelimiters:\n#     \
                      default: '[]'\n#     '%i':    '()'\n\n# good\n%w[alpha beta] + \
                      %i(gamma delta)\n\n# bad\n%W(alpha #{beta})\n\n# bad\n%I(alpha beta)\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::ArrayNode,
            NodeKind::RegularExpressionNode,
            NodeKind::InterpolatedRegularExpressionNode,
            NodeKind::StringNode,
            NodeKind::InterpolatedStringNode,
            NodeKind::SymbolNode,
            NodeKind::InterpolatedSymbolNode,
            NodeKind::XStringNode,
            NodeKind::InterpolatedXStringNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let raw = options.get("PreferredDelimiters").and_then(OptionValue::as_map);
        let has_default_key =
            raw.is_some_and(|entries| entries.iter().any(|(k, _)| k == "default"));
        let lookup = |key: &str| -> Option<&str> {
            raw.and_then(|entries| {
                entries.iter().find(|(k, _)| k == key).and_then(|(_, v)| v.as_str())
            })
        };

        let mut preferred = [(0u8, 0u8); 10];
        for (i, ty) in PERCENT_LITERAL_TYPES.iter().enumerate() {
            let value = if raw.is_some() {
                if has_default_key {
                    lookup(ty).or_else(|| lookup("default"))
                } else {
                    lookup(ty)
                }
            } else {
                None
            };
            let value = value.map_or_else(
                || {
                    DEFAULT_OVERRIDES
                        .iter()
                        .find(|(k, _)| k == ty)
                        .map_or(DEFAULT_FALLBACK, |(_, v)| v)
                },
                |v| v,
            );
            let bytes = value.as_bytes();
            preferred[i] = match bytes {
                [open, close, ..] => (*open, *close),
                [only] => (*only, *only),
                [] => (b'(', b')'),
            };
        }
        Ok(Self { preferred })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some((opening, closing, contents)) = literal_parts(node) else { return };

        let opening_text = ctx.text(opening.span());
        if opening_text.first() != Some(&b'%') || opening_text.len() < 2 {
            return;
        }
        let kind_bytes = &opening_text[..opening_text.len() - 1];
        let Ok(kind) = std::str::from_utf8(kind_bytes) else { return };
        let Some((preferred_open, preferred_close)) = self.preferred_for(kind) else { return };

        let used_open = *opening_text.last().expect("checked len >= 2");

        // `uses_preferred_delimiter?`.
        if used_open == preferred_open {
            return;
        }

        // `contains_preferred_delimiter?`.
        let contains = |bytes: &[u8], targets: &[u8]| targets.iter().any(|t| bytes.contains(t));
        let preferred_chars = [preferred_open, preferred_close];
        if contents.iter().any(|c| contains(c.bytes(ctx), &preferred_chars)) {
            return;
        }

        // `include_same_character_as_used_for_delimiter?`: only for the
        // exact `%w`/`%i` types (not their interpolated `%W`/`%I` forms).
        if kind == "%w" || kind == "%i" {
            let used_delimiters = matchpairs(used_open);
            if contents.iter().any(|c| contains(c.bytes(ctx), &used_delimiters)) {
                return;
            }
        }

        let message = format!(
            "`{kind}`-literals should be delimited by `{}` and `{}`.",
            preferred_open as char, preferred_close as char
        );

        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![
                Edit::replace(
                    opening.span(),
                    format!("{kind}{}", preferred_open as char).into_bytes(),
                ),
                Edit::replace(closing.span(), vec![preferred_close]),
            ],
        };
        ctx.report_with_fix(&Self::META, node.span(), message, fix);
    }
}

/// One piece of a percent literal's textual content, to be scanned for
/// delimiter characters. See the module doc for why the two shapes read
/// different byte ranges.
enum Contents<'pr> {
    /// A bare (non-array, non-interpolated) string literal: its decoded
    /// `unescaped()` content, computed lazily so the borrow never has to
    /// outlive the small typed node value stored here (which is `Copy`).
    UnescapedString(ruby_ast::node::StringNode<'pr>),
    /// Same as `UnescapedString`, for a bare symbol literal.
    UnescapedSymbol(ruby_ast::node::SymbolNode<'pr>),
    /// Same as `UnescapedString`, for a bare xstring literal.
    UnescapedXString(ruby_ast::node::XStringNode<'pr>),
    /// A nested `str`/`sym` AST node's raw source span (array elements,
    /// regexp content, and interpolated literals' literal fragments).
    Raw(ruby_source::Span),
}

impl Contents<'_> {
    fn bytes<'a>(&'a self, ctx: &'a Context<'_>) -> &'a [u8] {
        match self {
            Contents::UnescapedString(s) => s.unescaped(),
            Contents::UnescapedSymbol(s) => s.unescaped(),
            Contents::UnescapedXString(s) => s.unescaped(),
            Contents::Raw(span) => ctx.text(*span),
        }
    }
}

/// `string_source` applied to an interpolated literal's `parts`: the raw
/// source span of each literal `StringNode`/`SymbolNode` fragment: every
/// other part kind (embedded expressions, etc.) is not a `str`/`sym`-typed
/// node and is skipped, matching upstream's `filter_map`.
fn string_parts_raw<'pr>(parts: &[Node<'pr>]) -> Vec<Contents<'pr>> {
    parts
        .iter()
        .filter_map(|part| match part {
            Node::StringNode { .. } | Node::SymbolNode { .. } => Some(Contents::Raw(part.span())),
            _ => None,
        })
        .collect()
}

/// `PreferredDelimiters::PreferredDelimiters#matchpairs`: the matching pair
/// for a currently-used opening delimiter byte, or the byte alone when it
/// isn't one of the four bracket pairs.
fn matchpairs(begin_delimiter: u8) -> Vec<u8> {
    match begin_delimiter {
        b'(' => vec![b'(', b')'],
        b'[' => vec![b'[', b']'],
        b'{' => vec![b'{', b'}'],
        b'<' => vec![b'<', b'>'],
        other => vec![other],
    }
}
