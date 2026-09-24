//! `Style/EmptyElse`, ported from RuboCop's `lib/rubocop/cop/style/empty_else.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::ElseNode;
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Redundant `else`-clause.";

/// RuboCop's `NIL_STYLES`/`EMPTY_STYLES` collapsed into one `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Empty,
    Nil,
    Both,
}

impl Style {
    fn checks_empty(self) -> bool {
        matches!(self, Style::Empty | Style::Both)
    }

    fn checks_nil(self) -> bool {
        matches!(self, Style::Nil | Style::Both)
    }
}

/// Checks for empty `else`-clauses, possibly including comments and/or an
/// explicit `nil`, depending on `EnforcedStyle`.
#[derive(Debug, Clone)]
pub struct EmptyElse {
    style: Style,
    allow_comments: bool,
    /// RuboCop's `missing_else_style`: `Style/MissingElse`'s `EnforcedStyle`
    /// when that peer cop is enabled (`Enabled: true` in config), else
    /// `None` -- autocorrection is then never forbidden by it.
    missing_else_style: Option<String>,
}

impl EmptyElse {
    /// RuboCop's `autocorrect_forbidden?`: `[type, 'both'].include?(missing_else_style)`,
    /// where `type` is `"if"` for `if`/`unless` (whitequark unifies both under
    /// the `:if` node type) and `"case"` for `case`.
    fn autocorrect_forbidden(&self, type_str: &str) -> bool {
        self.missing_else_style.as_deref().is_some_and(|style| style == type_str || style == "both")
    }

    /// RuboCop's `check` plus `empty_check`/`nil_check`/`autocorrect`, run
    /// once the caller has located the real terminal `else` clause (never an
    /// `elsif` link, never a ternary's `:` branch).
    fn check(&mut self, ctx: &mut Context<'_>, else_node: &ElseNode<'_>, type_str: &str) {
        let full_span = else_node.location().span();
        let has_comment = has_comment_within(full_span, ctx);
        if self.allow_comments && has_comment {
            return;
        }

        let offense = match else_node.statements() {
            None => self.style.checks_empty(),
            Some(stmts) => {
                let body = stmts.body();
                self.style.checks_nil()
                    && body.len() == 1
                    && matches!(body.iter().next(), Some(Node::NilNode { .. }))
            }
        };
        if !offense {
            return;
        }

        let else_keyword_span = else_node.else_keyword_loc().span();
        if self.autocorrect_forbidden(type_str) || has_comment {
            ctx.report(&<Self as Rule>::META, else_keyword_span, MSG);
            return;
        }

        match else_node.end_keyword_loc() {
            Some(end_loc) => {
                let fix = Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::delete(Span::new(
                        else_keyword_span.start,
                        end_loc.span().start,
                    ))],
                };
                ctx.report_with_fix(&<Self as Rule>::META, else_keyword_span, MSG, fix);
            }
            // No terminal `end` (only reachable for a ternary's `:` branch,
            // already excluded before `check` is called): report without a
            // fix rather than guess at a deletion range.
            None => ctx.report(&<Self as Rule>::META, else_keyword_span, MSG),
        }
    }
}

impl Rule for EmptyElse {
    const META: RuleMeta = RuleMeta {
        name: "Style/EmptyElse",
        department: Department::Style,
        summary: "Checks for empty `else`-clauses, possibly including comments and/or an \
                  explicit `nil` depending on the `EnforcedStyle`.",
        explanation: "\
```ruby
# EnforcedStyle: both (default)
# warn on empty else and else with nil in it

# bad
if condition
  statement
else
  nil
end

# bad
if condition
  statement
else
end

# good
if condition
  statement
else
  statement
end

# good
if condition
  statement
end
```

With `EnforcedStyle: empty`, only a completely empty `else`-clause is flagged
(an explicit `nil` is allowed). With `EnforcedStyle: nil`, only an
`else`-clause whose sole statement is `nil` is flagged (a completely empty
one is allowed).

With `AllowComments: true`, an `else`-clause that carries a comment --
whether or not it is otherwise empty or `nil` -- is never flagged.

`if`, `unless`, and `case` are covered; `case`/`in` pattern matching is not
(see `blind_spots`).",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode, NodeKind::CaseNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("both"),
                allowed: &["empty", "nil", "both"],
                doc: "`empty` flags a completely empty `else`; `nil` flags an `else` whose \
                      sole statement is `nil`; `both` flags either.",
            },
            ConfigOption {
                name: "AllowComments",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Allow a comment inside an otherwise-empty/`nil` `else`-clause.",
            },
        ],
        blind_spots: "\
`case`/`in` pattern matching (Prism's `CaseMatchNode`) is never checked:
RuboCop's cop only defines `on_normal_if_unless` and `on_case`, so
`case/in` expressions never reach it (pattern matching already raises
`NoMatchingPatternError` without an `else`, per the cop's own class
comment) -- this is a deliberate false negative, not an oversight.

`Style/MissingElse`'s `EnforcedStyle` (read as a peer option, to decide
whether autocorrection is forbidden when that peer cop is enabled) always
resolves through this engine's `LoadedConfig`, which merges every cop's
settings with the bundled real `config/default.yml` -- so `EnforcedStyle`
reads as \"both\" (RuboCop's own documented default) whenever a fixture's
`.rubocop.yml` enables `Style/MissingElse` without naming `EnforcedStyle`
explicitly. RuboCop's own cop-spec unit tests build a bare
`RuboCop::Config.new(hash)` with no such default merge, so the same
`{'Enabled' => true}` peer hash resolves `EnforcedStyle` to `nil` there
instead, never forbidding autocorrection. This is a structural mismatch
between the fixtures' RSpec-derived ground truth and this engine's (more
realistic, CLI-accurate) config resolution, not something `configure`
can distinguish through the `RuleOptions::peer` API -- it affects the
`autocorrect_missingelse_is_disabled_does_autocorrection*` fixtures only.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "empty" => Style::Empty,
            "nil" => Style::Nil,
            _ => Style::Both,
        };
        let allow_comments = options.bool("AllowComments");
        let missing_else_enabled = options
            .peer("Style/MissingElse", "Enabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(false);
        let missing_else_style = missing_else_enabled.then(|| {
            options
                .peer("Style/MissingElse", "EnforcedStyle")
                .and_then(OptionValue::as_str)
                .unwrap_or("both")
                .to_string()
        });
        Ok(Self { style, allow_comments, missing_else_style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::IfNode { .. } => {
                let n = node.as_if_node().expect("kind matched");
                // `OnNormalIfUnless`: a ternary (no literal `if` keyword) is
                // never dispatched to this cop. Its own `subsequent` can
                // still resolve to an `ElseNode` (Prism reuses `ElseNode`
                // for a ternary's `:` branch), so this must run first.
                if n.if_keyword_loc().is_none() {
                    return;
                }
                // `subsequent` is either `None` (no `else`/`elsif` at all),
                // another `IfNode` (an `elsif` link visited independently
                // when the traversal reaches it), or the real terminal
                // `ElseNode` this `if`/`elsif` link owns.
                if let Some(else_node) = n.subsequent().and_then(|sub| sub.as_else_node()) {
                    self.check(ctx, &else_node, "if");
                }
            }
            Node::UnlessNode { .. } => {
                let n = node.as_unless_node().expect("kind matched");
                if let Some(else_node) = n.else_clause() {
                    self.check(ctx, &else_node, "if");
                }
            }
            Node::CaseNode { .. } => {
                let n = node.as_case_node().expect("kind matched");
                if let Some(else_node) = n.else_clause() {
                    self.check(ctx, &else_node, "case");
                }
            }
            _ => {}
        }
    }
}

/// RuboCop's `comment_in_else?` (approximated as "any comment fully inside
/// the else-clause's own span", which already runs from the `else` keyword
/// through the closing `end`).
fn has_comment_within(span: Span, ctx: &Context<'_>) -> bool {
    ctx.comments().iter().any(|c| c.span.start >= span.start && c.span.end <= span.end)
}
