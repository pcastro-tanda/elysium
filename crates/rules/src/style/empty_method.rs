//! `Style/EmptyMethod`, ported from RuboCop's
//! `lib/rubocop/cop/style/empty_method.rb`, plus the `ConfigurableEnforcedStyle`
//! mixin it includes and the `max_line_length` helper from
//! `AutocorrectLogic`.
//!
//! Prism unifies whitequark's `def`/`defs` (`on_def`/`on_defs`, aliased to the
//! same handler upstream) into a single `DefNode` with an optional
//! `receiver`, so one `NodeKind::DefNode` subscription covers both instance
//! and singleton method definitions. `node.body` maps directly to
//! `DefNode::body()`: `None` for a truly empty body (including a lone
//! `# comment`, which Prism -- like whitequark -- treats as no statements).
//! Endless methods (`def foo = expr`) always have `body: Some(_)` and are
//! skipped before either check. Upstream's `contains_comment?` check is
//! line-based (`each_comment_in_lines(range.line..range.last_line)`), so a
//! trailing comment on the `end` line -- or any other line the node spans --
//! also counts, not just comments whose byte range falls strictly inside
//! the node's own span.
//!
//! `corrected` rebuilds the parameter list from `ParametersNode`'s seven
//! separate buckets (`requireds`, `optionals`, `rest`, `posts`, `keywords`,
//! `keyword_rest`, `block`) walked in that fixed order -- the only legal
//! Ruby parameter order, so concatenating the buckets reproduces upstream's
//! `node.arguments.map(&:source).join(', ')` source order for free. Whether
//! that list is wrapped in parentheses comes straight off `DefNode`'s own
//! `lparen_loc`, unlike whitequark where `parentheses?` inspects the args
//! node's own `end` location.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::ParametersNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG_COMPACT`.
const MSG_COMPACT: &str = "Put empty method definitions on a single line.";
/// RuboCop's `MSG_EXPANDED`.
const MSG_EXPANDED: &str = "Put the `end` of empty method definitions on the next line.";

/// Checks the formatting of empty method definitions.
#[derive(Debug, Clone)]
pub struct EmptyMethod {
    /// RuboCop's `compact_style?` (the raw `EnforcedStyle` option).
    compact_style: bool,
    /// RuboCop's `compact_style_disallowed?`: `compact` style is configured
    /// but `Style/SingleLineMethods` is enabled with `AllowIfMethodIsEmpty:
    /// false`, so no compact empty method is ever acceptable and this cop
    /// must not add offenses that `SingleLineMethods` would just re-flag
    /// the other way.
    compact_style_disallowed: bool,
    /// RuboCop's `AutocorrectLogic#max_line_length`: `Layout/LineLength`'s
    /// configured `Max`, or `None` when that cop is disabled.
    max_line_length: Option<i64>,
}

impl EmptyMethod {
    /// RuboCop's `corrected`.
    fn corrected(&self, node: &Node<'_>, ctx: &Context<'_>) -> String {
        let def = node.as_def_node().expect("kind matched");
        let scope = def
            .receiver()
            .map(|r| format!("{}.", String::from_utf8_lossy(ctx.text(r.span()))))
            .unwrap_or_default();
        let params = def_parameter_list(def.parameters());
        let arguments = if params.is_empty() {
            String::new()
        } else {
            let args = params
                .iter()
                .map(|n| String::from_utf8_lossy(ctx.text(n.span())).into_owned())
                .collect::<Vec<_>>()
                .join(", ");
            if def.lparen_loc().is_some() {
                format!("({args})")
            } else {
                format!(" {args}")
            }
        };
        let method_name = String::from_utf8_lossy(def.name().as_slice());
        let signature = format!("{scope}{method_name}{arguments}");
        let joint = if self.compact_style {
            "; ".to_string()
        } else {
            let column = ctx.line_col(node.span().start).column;
            format!("\n{}", " ".repeat(column as usize))
        };
        format!("def {signature}{joint}end")
    }
}

impl Rule for EmptyMethod {
    const META: RuleMeta = RuleMeta {
        name: "Style/EmptyMethod",
        department: Department::Style,
        summary: "Checks the formatting of empty method definitions.",
        explanation: "\
Checks for the formatting of empty method definitions. By default it \
enforces empty method definitions to go on a single line (`compact` style), \
but it can be configured to enforce the `end` to go on its own line \
(`expanded` style).

A method definition is not considered empty if it contains comments.

Autocorrection will not be applied for the `compact` style if the resulting \
code is longer than the `Max` configuration for `Layout/LineLength`, but an \
offense will still be registered.

```ruby
# EnforcedStyle: compact (default)

# bad
def foo(bar)
end

def self.foo(bar)
end

# good
def foo(bar); end

def foo(bar)
  # baz
end

def self.foo(bar); end

# EnforcedStyle: expanded

# bad
def foo(bar); end

def self.foo(bar); end

# good
def foo(bar)
end

def self.foo(bar)
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("compact"),
            allowed: &["compact", "expanded"],
            doc: "Whether an empty method definition's `end` should share the `def` line \
                  (`compact`) or go on its own line (`expanded`).",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let compact_style = options.style("EnforcedStyle")? == "compact";
        let single_line_methods_enabled = options
            .peer("Style/SingleLineMethods", "Enabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(true);
        let allow_if_method_is_empty = options
            .peer("Style/SingleLineMethods", "AllowIfMethodIsEmpty")
            .and_then(OptionValue::as_bool);
        let compact_style_disallowed =
            compact_style && single_line_methods_enabled && allow_if_method_is_empty == Some(false);
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
        Ok(Self { compact_style, compact_style_disallowed, max_line_length })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(def) = node.as_def_node() else { return };
        if def.body().is_some() {
            return;
        }
        let span = node.span();
        let start_line = ctx.line_col(span.start).line;
        let last_line = ctx.last_line(span);
        if ctx.comments().iter().any(|c| (start_line..=last_line).contains(&c.line)) {
            return;
        }
        let single_line = ctx.is_single_line(span);
        if self.compact_style {
            if single_line || self.compact_style_disallowed {
                return;
            }
        } else if !single_line {
            return;
        }
        let message = if self.compact_style { MSG_COMPACT } else { MSG_EXPANDED };
        let correction = self.corrected(node, ctx);
        let exceeds_line_length = self.compact_style
            && self
                .max_line_length
                .is_some_and(|max| i64::try_from(correction.len()).unwrap_or(i64::MAX) > max);
        if exceeds_line_length {
            ctx.report(&Self::META, span, message);
        } else {
            ctx.report_with_fix(
                &Self::META,
                span,
                message,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(span, correction.into_bytes())],
                },
            );
        }
    }
}

/// Every entry in a `def`'s parameter list, in declaration order. RuboCop's
/// `node.arguments` for a `DefNode`/`DefsNode`, flattened from Prism's
/// per-kind buckets.
fn def_parameter_list(params: Option<ParametersNode<'_>>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    let Some(params) = params else { return out };
    out.extend(params.requireds().iter());
    out.extend(params.optionals().iter());
    if let Some(rest) = params.rest() {
        out.push(rest);
    }
    out.extend(params.posts().iter());
    out.extend(params.keywords().iter());
    if let Some(kwrest) = params.keyword_rest() {
        out.push(kwrest);
    }
    if let Some(block) = params.block() {
        out.push(block.as_node());
    }
    out
}
