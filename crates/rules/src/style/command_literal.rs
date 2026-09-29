//! `Style/CommandLiteral`, ported from RuboCop's
//! `lib/rubocop/cop/style/command_literal.rb`.
//!
//! Prism gives every backtick/`%x` command literal a
//! [`NodeKind::XStringNode`] (no interpolation) or
//! [`NodeKind::InterpolatedXStringNode`] (has interpolation); both map onto
//! upstream's single `on_xstr` entry point. The preferred delimiter comes
//! from `Style/PercentLiteralDelimiters`'s `PreferredDelimiters` option,
//! read as a peer cop setting (`RuleOptions::peer`) exactly like RuboCop's
//! `config.for_cop('Style/PercentLiteralDelimiters')`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

const MSG_USE_BACKTICKS: &str = "Use backticks around command string.";
const MSG_USE_PERCENT_X: &str = "Use `%x` around command string.";

/// RuboCop's `ConfigurableEnforcedStyle` `style` for this cop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// Always use backticks.
    Backticks,
    /// Always use `%x`.
    PercentX,
    /// Backticks on single-line commands, `%x` on multi-line ones.
    Mixed,
}

/// Use backticks or `%x` around command literals.
#[derive(Debug, Clone)]
pub struct CommandLiteral {
    style: Style,
    allow_inner_backticks: bool,
    /// `%x`'s preferred delimiter pair, from
    /// `Style/PercentLiteralDelimiters`'s `PreferredDelimiters` (its own
    /// `%x` entry, else its `default` entry, else `()`).
    delimiters: (u8, u8),
}

impl Rule for CommandLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Style/CommandLiteral",
        department: Department::Style,
        summary: "Use `` or %x around command literals.",
        explanation: "Enforces using `` or %x around command literals.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::XStringNode, NodeKind::InterpolatedXStringNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("backticks"),
                allowed: &["backticks", "percent_x", "mixed"],
                doc: "The preferred style for command literals.",
            },
            ConfigOption {
                name: "AllowInnerBackticks",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether an inner backtick is allowed to stay as a backtick literal \
                      instead of forcing `%x`.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "percent_x" => Style::PercentX,
            "mixed" => Style::Mixed,
            _ => Style::Backticks,
        };
        let allow_inner_backticks = options.bool("AllowInnerBackticks");
        let delimiters = preferred_delimiters(options);
        Ok(Self { style, allow_inner_backticks, delimiters })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if ruby_ast::ext::is_heredoc(node) {
            return;
        }
        let (node_span, opening_span, closing_span) = match node {
            Node::XStringNode { .. } => {
                let n = node.as_x_string_node().expect("kind matched");
                (n.location().span(), n.opening_loc().span(), n.closing_loc().span())
            }
            Node::InterpolatedXStringNode { .. } => {
                let n = node.as_interpolated_x_string_node().expect("kind matched");
                (n.location().span(), n.opening_loc().span(), n.closing_loc().span())
            }
            _ => return,
        };

        // RuboCop's `node_body`: the raw source between the opening and
        // closing delimiters, interpolation included verbatim.
        let body = ctx.text(Span::new(opening_span.end, closing_span.start));
        let contains_backtick = body.contains(&b'`');
        let disallowed_backtick = !self.allow_inner_backticks && contains_backtick;
        let backtick_literal = ctx.text(opening_span) == b"`";
        let multiline = !ctx.is_single_line(node_span);

        let (allowed, message) = if backtick_literal {
            let allowed = match self.style {
                Style::Backticks => !disallowed_backtick,
                Style::Mixed => !multiline && !disallowed_backtick,
                Style::PercentX => false,
            };
            (allowed, MSG_USE_PERCENT_X)
        } else {
            let allowed = match self.style {
                Style::Backticks => disallowed_backtick,
                Style::Mixed => multiline || disallowed_backtick,
                Style::PercentX => true,
            };
            (allowed, MSG_USE_BACKTICKS)
        };
        if allowed {
            return;
        }

        // RuboCop's `autocorrect`: bails out (offense still reported, just
        // without a fix) whenever the body contains any backtick at all,
        // regardless of `AllowInnerBackticks` -- swapping delimiters around
        // a literal backtick is not a safe rewrite.
        if contains_backtick {
            ctx.report(&Self::META, node_span, message);
            return;
        }

        let mut opening_replacement = Vec::with_capacity(3);
        let mut closing_replacement = Vec::with_capacity(1);
        if backtick_literal {
            let (open, close) = self.delimiters;
            opening_replacement.extend_from_slice(b"%x");
            opening_replacement.push(open);
            closing_replacement.push(close);
        } else {
            opening_replacement.push(b'`');
            closing_replacement.push(b'`');
        }
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![
                Edit::replace(opening_span, opening_replacement),
                Edit::replace(closing_span, closing_replacement),
            ],
        };
        ctx.report_with_fix(&Self::META, node_span, message, fix);
    }
}

/// `Style/PercentLiteralDelimiters`'s `PreferredDelimiters` option, read as
/// a peer cop setting: its own `%x` entry, else its `default` entry, else
/// `()` (matching `config/default.yml`'s own fallback).
fn preferred_delimiters(options: &RuleOptions) -> (u8, u8) {
    let map = options.peer("Style/PercentLiteralDelimiters", "PreferredDelimiters");
    let entries = map.and_then(OptionValue::as_map).unwrap_or(&[]);
    let lookup =
        |key: &str| entries.iter().find(|(name, _)| name == key).and_then(|(_, v)| v.as_str());
    let delimiter = lookup("%x").or_else(|| lookup("default")).unwrap_or("()");
    let mut bytes = delimiter.bytes();
    let open = bytes.next().unwrap_or(b'(');
    let close = bytes.next().unwrap_or(open);
    (open, close)
}
