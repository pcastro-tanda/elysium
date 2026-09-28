//! `Style/TernaryParentheses`, ported from RuboCop's
//! `lib/rubocop/cop/style/ternary_parentheses.rb` plus the `SafeAssignment`
//! mixin it includes.
//!
//! A ternary is Prism's `IfNode` with no `if_keyword_loc` and a
//! `then_keyword_loc` whose text is literally `?` (as opposed to a modifier
//! `if`, which has neither keyword loc, or an explicit one-line `if ... then
//! ...`, whose `then_keyword_loc` text is `then`). `node.condition` is
//! `IfNode::predicate`.
//!
//! Whitequark's `begin` node (any parenthesized/grouped expression) is
//! Prism's `ParenthesesNode`; its single wrapped statement is always
//! `ParenthesesNode::body`'s `StatementsNode`, even when there is exactly one
//! statement (unlike whitequark, which elides the wrapper). Every
//! `condition.begin_type?`/`condition.children` check below therefore goes
//! through `ParenthesesNode::body` -> `StatementsNode::body`.
//!
//! Whitequark's unary `not`/`!` (a `send` node with the operand as its own
//! `receiver` and method name `:!`, distinguished only by whether
//! `loc.selector` reads `not` or `!`) matches Prism's `CallNode` exactly:
//! `receiver` is the operand, `name` is `!`, and `message_loc` covers `not`
//! or `!` textually. `not foo ? a : b` parses with `not` at *lower*
//! precedence than the ternary (Prism nests the `IfNode` inside `not`'s
//! `CallNode` as its receiver), so a bare leading `not`/`!` never appears
//! inside `node.condition` itself -- only a parenthesized one does, e.g.
//! `(not foo) ? a : b`.
//!
//! `and`/`or` (word form) vs `&&`/`||` (symbol form) both parse to Prism's
//! `AndNode`/`OrNode`; only `operator_loc`'s text distinguishes them,
//! matching RuboCop's `semantic_operator?`.
//!
//! One-line pattern matching (`expr in pattern`) is Prism's
//! `MatchPredicateNode`; the older Ruby 2.7-only rightward form (`expr =>
//! pattern`) is `MatchRequiredNode`. `target_ruby_version` selects between
//! them exactly as upstream does.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Parentheses,
    NoParentheses,
    WhenComplex,
}

/// Checks for the presence of parentheses around ternary conditions. It is
/// configurable to enforce inclusion or omission of parentheses using
/// `EnforcedStyle`. Omission is only enforced when removing the parentheses
/// won't cause a different behavior.
#[derive(Debug, Clone)]
pub struct TernaryParentheses {
    style: Style,
    allow_safe_assignment: bool,
    target_ruby_version: f32,
}

impl Rule for TernaryParentheses {
    const META: RuleMeta = RuleMeta {
        name: "Style/TernaryParentheses",
        department: Department::Style,
        summary: "Checks for use of parentheses around ternary conditions.",
        explanation: "\
Checks for the presence of parentheses around ternary
conditions. It is configurable to enforce inclusion or omission of
parentheses using `EnforcedStyle`. Omission is only enforced when
removing the parentheses won't cause a different behavior.

`AllowSafeAssignment` option for safe assignment.
By safe assignment we mean putting parentheses around
an assignment to indicate \"I know I'm using an assignment
as a condition. It's not a mistake.\"

```ruby
# EnforcedStyle: require_no_parentheses (default)
# bad
foo = (bar?) ? a : b
foo = (bar.baz?) ? a : b
foo = (bar && baz) ? a : b

# good
foo = bar? ? a : b
foo = bar.baz? ? a : b
foo = bar && baz ? a : b
```

```ruby
# EnforcedStyle: require_parentheses
# bad
foo = bar? ? a : b
foo = bar.baz? ? a : b
foo = bar && baz ? a : b

# good
foo = (bar?) ? a : b
foo = (bar.baz?) ? a : b
foo = (bar && baz) ? a : b
```

```ruby
# EnforcedStyle: require_parentheses_when_complex
# bad
foo = (bar?) ? a : b
foo = (bar.baz?) ? a : b
foo = bar && baz ? a : b

# good
foo = bar? ? a : b
foo = bar.baz? ? a : b
foo = (bar && baz) ? a : b
```

```ruby
# AllowSafeAssignment: true (default)
# good
foo = (bar = baz) ? a : b
```

```ruby
# AllowSafeAssignment: false
# bad
foo = (bar = baz) ? a : b
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("require_no_parentheses"),
                allowed: &[
                    "require_parentheses",
                    "require_no_parentheses",
                    "require_parentheses_when_complex",
                ],
                doc: "Whether ternary conditions must, must not, or must (only when complex) \
                      be wrapped in parentheses.",
            },
            ConfigOption {
                name: "AllowSafeAssignment",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether an assignment wrapped in parentheses (`(bar = baz) ? a : b`) is \
                      allowed as a deliberate \"I know I'm using an assignment\" marker.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "require_parentheses" => Style::Parentheses,
            "require_parentheses_when_complex" => Style::WhenComplex,
            _ => Style::NoParentheses,
        };
        let allow_safe_assignment = options.bool("AllowSafeAssignment");
        let target_ruby_version = options.target_ruby_version();
        Ok(Self { style, allow_safe_assignment, target_ruby_version })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(if_node) = node.as_if_node() else { return };
        if if_node.if_keyword_loc().is_some() {
            return;
        }
        let Some(then_loc) = if_node.then_keyword_loc() else { return };
        if ctx.text(then_loc.span()) != b"?" {
            return;
        }

        let condition = if_node.predicate();

        if only_closing_paren_last_line(ctx, condition.span()) {
            return;
        }
        if is_one_line_pattern_match(&condition, ctx, self.target_ruby_version) {
            return;
        }

        let safe_assignment = is_safe_assignment(&condition);
        let parens = is_parenthesized(&condition);
        let offense = if safe_assignment {
            !self.allow_safe_assignment
        } else {
            match self.style {
                Style::WhenComplex => {
                    if is_complex_condition(&condition) {
                        !parens
                    } else {
                        parens
                    }
                }
                Style::Parentheses => !parens,
                Style::NoParentheses => parens,
            }
        };
        if !offense {
            return;
        }

        let message = self.message(parens);
        let span = node.span();

        match autocorrect(ctx, &condition, parens, safe_assignment) {
            Some(edits) => {
                let fix = Fix { applicability: Applicability::Safe, edits };
                ctx.report_with_fix(&Self::META, span, message, fix);
            }
            None => ctx.report(&Self::META, span, message),
        }
    }
}

impl TernaryParentheses {
    /// RuboCop's `message`.
    fn message(&self, parens: bool) -> String {
        if self.style == Style::WhenComplex {
            let command = if parens { "Only use" } else { "Use" };
            format!("{command} parentheses for ternary expressions with complex conditions.")
        } else {
            let command = if self.style == Style::Parentheses { "Use" } else { "Omit" };
            format!("{command} parentheses for ternary conditions.")
        }
    }
}

/// RuboCop's `autocorrect`. `None` means the corrector block returned
/// `nil` (no fix, offense-only).
fn autocorrect(
    ctx: &Context<'_>,
    condition: &Node<'_>,
    parens: bool,
    safe_assignment: bool,
) -> Option<Vec<Edit>> {
    if parens && (safe_assignment || is_unsafe_autocorrect(condition, ctx)) {
        return None;
    }
    Some(if parens {
        correct_parenthesized(ctx, condition)
    } else {
        correct_unparenthesized(condition)
    })
}

/// RuboCop's `only_closing_parenthesis_is_last_line?`.
fn only_closing_paren_last_line(ctx: &Context<'_>, span: Span) -> bool {
    ctx.text(span).rsplit(|&b| b == b'\n').next() == Some(&b")"[..])
}

/// RuboCop's `condition_as_parenthesized_one_line_pattern_matching?`.
fn is_one_line_pattern_match(
    condition: &Node<'_>,
    _ctx: &Context<'_>,
    target_ruby_version: f32,
) -> bool {
    let Some(paren) = condition.as_parentheses_node() else { return false };
    let Some(first) = first_paren_child(&paren) else { return false };
    if target_ruby_version >= 3.0 {
        first.as_match_predicate_node().is_some()
    } else {
        first.as_match_required_node().is_some()
    }
}

/// RuboCop's `parenthesized?`.
fn is_parenthesized(condition: &Node<'_>) -> bool {
    condition.as_parentheses_node().is_some()
}

/// The direct children of a parenthesized condition -- RuboCop's
/// `condition.to_a`/`condition.children` on a `begin` node.
fn paren_children<'pr>(paren: &ruby_ast::node::ParenthesesNode<'pr>) -> Vec<Node<'pr>> {
    paren
        .body()
        .and_then(|body| body.as_statements_node())
        .map(|stmts| stmts.body().iter().collect())
        .unwrap_or_default()
}

fn first_paren_child<'pr>(paren: &ruby_ast::node::ParenthesesNode<'pr>) -> Option<Node<'pr>> {
    paren.body().and_then(|body| body.as_statements_node()).and_then(|stmts| stmts.body().first())
}

/// RuboCop's `SafeAssignment#safe_assignment?`: `(begin {equals_asgn?
/// #setter_method?})` -- a parenthesized condition whose single child is an
/// assignment (`lvasgn`/`ivasgn`/`cvasgn`/`gvasgn`/`casgn`/`masgn`, i.e. not
/// an `op_asgn`/`or_asgn`/`and_asgn` shorthand) or a setter-method call
/// (`foo.bar = baz`, recognised in Prism by `CallNode::equal_loc`).
fn is_safe_assignment(condition: &Node<'_>) -> bool {
    let Some(paren) = condition.as_parentheses_node() else { return false };
    let children = paren_children(&paren);
    let [child] = children.as_slice() else { return false };
    is_equals_asgn(child) || is_setter_method(child)
}

fn is_equals_asgn(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::LocalVariableWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::MultiWriteNode
    )
}

fn is_setter_method(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|call| call.equal_loc().is_some())
}

/// RuboCop's `complex_condition?`, recursing into a parenthesized
/// condition's children.
fn is_complex_condition(condition: &Node<'_>) -> bool {
    if let Some(paren) = condition.as_parentheses_node() {
        paren_children(&paren).iter().any(is_complex_condition)
    } else {
        !is_non_complex_expression(condition)
    }
}

/// RuboCop's `non_complex_expression?`: variables, constants, `defined?`,
/// `yield`, or a non-operator method call (`[]` counts as non-operator here
/// too, matching `non_complex_send?`).
fn is_non_complex_expression(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::LocalVariableReadNode
            | NodeKind::InstanceVariableReadNode
            | NodeKind::ClassVariableReadNode
            | NodeKind::GlobalVariableReadNode
            | NodeKind::ConstantReadNode
            | NodeKind::ConstantPathNode
            | NodeKind::DefinedNode
            | NodeKind::YieldNode
    ) || is_non_complex_send(node)
}

/// RuboCop's `non_complex_send?`.
fn is_non_complex_send(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    let name = call.name();
    let name = name.as_slice();
    !is_operator_method(name) || name == b"[]"
}

/// RuboCop-ast's `OPERATOR_METHODS`.
fn is_operator_method(name: &[u8]) -> bool {
    matches!(
        name,
        b"|" | b"^"
            | b"&"
            | b"<=>"
            | b"=="
            | b"==="
            | b"=~"
            | b">"
            | b">="
            | b"<"
            | b"<="
            | b"<<"
            | b">>"
            | b"+"
            | b"-"
            | b"*"
            | b"/"
            | b"%"
            | b"**"
            | b"~"
            | b"+@"
            | b"-@"
            | b"!@"
            | b"~@"
            | b"[]"
            | b"[]="
            | b"!"
            | b"!="
            | b"!~"
            | b"`"
    )
}

/// RuboCop's `unsafe_autocorrect?`: does any child of an already-parenthesized
/// condition bind *looser* than the ternary operator, so that removing the
/// parentheses would change what they group?
fn is_unsafe_autocorrect(condition: &Node<'_>, ctx: &Context<'_>) -> bool {
    let Some(paren) = condition.as_parentheses_node() else { return false };
    paren_children(&paren).iter().any(|child| is_below_ternary_precedence(child, ctx))
}

fn is_below_ternary_precedence(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    if let Some(or) = node.as_or_node() {
        return ctx.text(or.operator_loc().span()) == b"or";
    }
    if let Some(and) = node.as_and_node() {
        return ctx.text(and.operator_loc().span()) == b"and";
    }
    is_prefix_not(node, ctx)
}

/// RuboCop's `SendNode#prefix_not?`: a unary `!`/`not` call (represented in
/// Prism, like whitequark, with the operand as `receiver` and `name` `!`)
/// spelled with the low-precedence keyword form.
fn is_prefix_not(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.receiver().is_none() || call.name().as_slice() != b"!" {
        return false;
    }
    call.message_loc().is_some_and(|loc| ctx.text(loc.span()) == b"not")
}

/// RuboCop's `correct_unparenthesized`.
fn correct_unparenthesized(condition: &Node<'_>) -> Vec<Edit> {
    let span = condition.span();
    vec![Edit::insert(span.start, b"(".to_vec()), Edit::insert(span.end, b")".to_vec())]
}

/// RuboCop's `correct_parenthesized`.
fn correct_parenthesized(ctx: &Context<'_>, condition: &Node<'_>) -> Vec<Edit> {
    let paren = condition.as_parentheses_node().expect("parenthesized condition");
    let mut edits =
        vec![Edit::delete(paren.opening_loc().span()), Edit::delete(paren.closing_loc().span())];

    let end = paren.closing_loc().span().end;
    if !ctx.text(Span::new(end, end + 1)).first().is_some_and(|&b| b.is_ascii_whitespace()) {
        edits.push(Edit::insert(end, b" ".to_vec()));
    }

    if let Some(inner) = paren_children(&paren).last() {
        if node_args_need_parens(inner, ctx) {
            parenthesize_condition_arguments(inner, &mut edits);
        }
    }
    edits
}

/// RuboCop's `node_args_need_parens?`.
fn node_args_need_parens(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let (args, parenthesized, dot, safe_nav, alpha_name) = if let Some(call) = node.as_call_node() {
        let args: Vec<Node<'_>> =
            call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
        let dot = call.call_operator_loc().is_some_and(|loc| ctx.text(loc.span()) == b".");
        let name = call.name();
        let alpha = name.as_slice().first().is_some_and(u8::is_ascii_alphabetic);
        (args, call.closing_loc().is_some(), dot, call.is_safe_navigation(), alpha)
    } else if let Some(def) = node.as_defined_node() {
        (vec![def.value()], def.rparen_loc().is_some(), false, false, true)
    } else {
        return false;
    };

    if args.is_empty() || parenthesized {
        return false;
    }
    dot || safe_nav || alpha_name
}

/// RuboCop's `parenthesize_condition_arguments`.
fn parenthesize_condition_arguments(node: &Node<'_>, edits: &mut Vec<Edit>) {
    let (keyword_end, first_start, last_end) = if let Some(call) = node.as_call_node() {
        let Some(message) = call.message_loc() else { return };
        let args: Vec<Node<'_>> =
            call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
        let (Some(first), Some(last)) = (args.first(), args.last()) else { return };
        (message.span().end, first.span().start, last.span().end)
    } else if let Some(def) = node.as_defined_node() {
        let value = def.value();
        (def.keyword_loc().span().end, value.span().start, value.span().end)
    } else {
        return;
    };

    edits.push(Edit::replace(Span::new(keyword_end, first_start), b"(".to_vec()));
    edits.push(Edit::insert(last_end, b")".to_vec()));
}
