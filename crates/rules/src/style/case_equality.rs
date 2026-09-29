//! `Style/CaseEquality`, ported from RuboCop's
//! `lib/rubocop/cop/style/case_equality.rb`.
//!
//! Upstream's `case_equality?` node-pattern `(send $#offending_receiver? :===
//! $_)` only matches a plain (never safe-navigation) `send` node with a
//! receiver and exactly one argument -- mirrored here as a `CallNode` guard
//! (`!is_safe_navigation()`, `receiver().is_some()`, exactly one argument).
//!
//! whitequark's `begin` node (an explicitly parenthesized group, e.g.
//! `(1..10)`) is Prism's [`NodeKind::ParenthesesNode`]; its single-statement
//! `StatementsNode` body is unwrapped to reach the same inner node
//! whitequark's `children.first` would see (see the module docs of
//! `redundant_parentheses.rs` for the general Prism/whitequark parens shape
//! difference). whitequark's unified `:const` node type (covering both a bare
//! `ConstantReadNode` and a namespaced `ConstantPathNode`) is handled by
//! matching both kinds; `module_name?` (does the constant's own last segment
//! contain a lowercase letter, i.e. does it look like a class/module name
//! rather than a `SCREAMING_SNAKE_CASE` constant) reads that same rightmost
//! segment either way.
//!
//! `unary_operation?` (used by `requires_parentheses?`) is `operator_method?
//! && loc.expression.begin_pos == selector.begin_pos`: true for a prefix
//! `-x`/`+x`/`!x`/`~x` spelling, where Prism's `CallNode` span starts at the
//! operator itself rather than at the receiver -- copied privately from
//! `redundant_parentheses.rs`'s identical check.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str = "Avoid the use of the case equality operator `===`.";

/// RuboCop-AST's `MethodIdentifierPredicates::OPERATOR_METHODS`.
const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

/// Avoid explicit use of the case equality operator (`===`).
#[derive(Debug, Clone)]
pub struct CaseEquality {
    allow_on_constant: bool,
    allow_on_self_class: bool,
}

impl Rule for CaseEquality {
    const META: RuleMeta = RuleMeta {
        name: "Style/CaseEquality",
        department: Department::Style,
        summary: "Avoid explicit use of the case equality operator (`===`).",
        explanation: "The `===` operator has different behavior depending on the receiver and \
            its use outside of `case`/`when` is confusing. Prefer more explicit alternatives \
            like `is_a?`, `include?`, or `match?`.\n\nIf `AllowOnConstant` is enabled, the cop \
            ignores violations when the receiver of the case equality operator is a constant. \
            If `AllowOnSelfClass` is enabled, the cop ignores violations when the receiver is \
            `self.class`.\n\nRegexp case equality (`/regexp/ === var`) is always allowed, since \
            rewriting it to `/regexp/.match?(var)` would need to account for \
            `Regexp.last_match?`, `$~`, `$1`, etc.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[
            linter::ConfigOption {
                name: "AllowOnConstant",
                default: linter::ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether the case equality operator is allowed when its receiver is a \
                    constant.",
            },
            linter::ConfigOption {
                name: "AllowOnSelfClass",
                default: linter::ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether the case equality operator is allowed when its receiver is \
                    `self.class`.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allow_on_constant: options.bool("AllowOnConstant"),
            allow_on_self_class: options.bool("AllowOnSelfClass"),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() || call.name().as_slice() != b"===" {
            return;
        }
        let Some(lhs) = call.receiver() else { return };
        let Some(args) = call.arguments() else { return };
        if args.arguments().len() != 1 {
            return;
        }
        let rhs = args.arguments().first().expect("checked len == 1");
        let Some(message_loc) = call.message_loc() else { return };

        if !self.offending_receiver(&lhs) {
            return;
        }
        if is_regexp(&lhs) || (is_const(&lhs) && !is_module_name(&lhs)) {
            return;
        }

        let span = message_loc.span();
        match replacement(ctx, &lhs, &rhs) {
            Some(text) => {
                let fix = Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(call.as_node().span(), text.into_bytes())],
                };
                ctx.report_with_fix(&Self::META, span, MSG, fix);
            }
            None => ctx.report(&Self::META, span, MSG),
        }
    }
}

impl CaseEquality {
    /// RuboCop's `offending_receiver?`.
    fn offending_receiver(&self, lhs: &Node<'_>) -> bool {
        if is_const(lhs) && self.allow_on_constant {
            return false;
        }
        if self.allow_on_self_class && is_self_class(lhs) {
            return false;
        }
        true
    }
}

/// RuboCop-AST's `Node#const_type?`, covering both a bare `ConstantReadNode`
/// and a namespaced `ConstantPathNode` (whitequark unifies both under a
/// single `:const` node type).
fn is_const(node: &Node<'_>) -> bool {
    node.as_constant_read_node().is_some() || node.as_constant_path_node().is_some()
}

/// RuboCop's `regexp_type?`: a regexp literal, with or without interpolation.
fn is_regexp(node: &Node<'_>) -> bool {
    node.as_regular_expression_node().is_some()
        || node.as_interpolated_regular_expression_node().is_some()
}

/// `rubocop-ast`'s `ConstantNode#module_name?`: does the constant's own
/// (rightmost) segment contain a lowercase letter, i.e. does it look like a
/// class/module name rather than a `SCREAMING_SNAKE_CASE` constant.
fn is_module_name(node: &Node<'_>) -> bool {
    let short_name = if let Some(c) = node.as_constant_read_node() {
        Some(c.name().as_slice())
    } else {
        node.as_constant_path_node().and_then(|p| p.name().map(|n| n.as_slice()))
    };
    short_name.is_some_and(|name| name.iter().any(u8::is_ascii_lowercase))
}

/// RuboCop's `self_class?` node-matcher: `(send (self) :class)`, a
/// receiver-less-args call to `class` on a bare `self` receiver.
fn is_self_class(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    !call.is_safe_navigation()
        && call.name().as_slice() == b"class"
        && call.arguments().is_none()
        && call.receiver().is_some_and(|r| r.as_self_node().is_some())
}

/// RuboCop's `replacement`.
fn replacement(ctx: &Context<'_>, lhs: &Node<'_>, rhs: &Node<'_>) -> Option<String> {
    if let Some(parens) = lhs.as_parentheses_node() {
        let stmts = parens.body()?.as_statements_node()?;
        let first = stmts.body().first()?;
        first.as_range_node()?;
        let lhs_text = String::from_utf8_lossy(ctx.text(parens.as_node().span()));
        let rhs_text = String::from_utf8_lossy(ctx.text(rhs.span()));
        return Some(format!("{lhs_text}.include?({rhs_text})"));
    }
    if is_const(lhs) {
        let lhs_text = String::from_utf8_lossy(ctx.text(lhs.span()));
        return Some(format!("{}.is_a?({lhs_text})", parenthesize_if_needed(ctx, rhs)));
    }
    if lhs.as_call_node().is_some() {
        if !is_self_class(lhs) {
            return None;
        }
        let lhs_text = String::from_utf8_lossy(ctx.text(lhs.span()));
        return Some(format!("{}.is_a?({lhs_text})", parenthesize_if_needed(ctx, rhs)));
    }
    None
}

/// RuboCop's `parenthesize_if_needed`.
fn parenthesize_if_needed(ctx: &Context<'_>, node: &Node<'_>) -> String {
    let text = String::from_utf8_lossy(ctx.text(node.span())).into_owned();
    if requires_parentheses(node) {
        format!("({text})")
    } else {
        text
    }
}

/// RuboCop's `requires_parentheses?`.
fn requires_parentheses(node: &Node<'_>) -> bool {
    if matches!(
        node.kind(),
        NodeKind::AndNode | NodeKind::OrNode | NodeKind::IfNode | NodeKind::RangeNode
    ) || is_assignment(node)
    {
        return true;
    }
    let Some(call) = node.as_call_node() else { return false };
    // RuboCop's `node.send_type? && (node.operator_method? ||
    // node.unary_operation?)`: `unary_operation?` already implies
    // `operator_method?`, so this collapses to just `operator_method?`.
    OPERATOR_METHODS.contains(&call.name().as_slice())
}

/// RuboCop-AST's `Node::ASSIGNMENTS` (`EQUALS_ASSIGNMENTS` +
/// `SHORTHAND_ASSIGNMENTS`): every `lvasgn`/`ivasgn`/`cvasgn`/`gvasgn`/
/// `casgn`/`masgn`/`op_asgn`/`or_asgn`/`and_asgn` shape, mapped onto Prism's
/// per-target-kind `*WriteNode` family.
fn is_assignment(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::LocalVariableWriteNode
            | NodeKind::LocalVariableAndWriteNode
            | NodeKind::LocalVariableOrWriteNode
            | NodeKind::LocalVariableOperatorWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::InstanceVariableAndWriteNode
            | NodeKind::InstanceVariableOrWriteNode
            | NodeKind::InstanceVariableOperatorWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::ClassVariableAndWriteNode
            | NodeKind::ClassVariableOrWriteNode
            | NodeKind::ClassVariableOperatorWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::GlobalVariableOperatorWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantAndWriteNode
            | NodeKind::ConstantOrWriteNode
            | NodeKind::ConstantOperatorWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::ConstantPathAndWriteNode
            | NodeKind::ConstantPathOrWriteNode
            | NodeKind::ConstantPathOperatorWriteNode
            | NodeKind::MultiWriteNode
            | NodeKind::CallOperatorWriteNode
            | NodeKind::CallAndWriteNode
            | NodeKind::CallOrWriteNode
            | NodeKind::IndexOperatorWriteNode
            | NodeKind::IndexAndWriteNode
            | NodeKind::IndexOrWriteNode
    )
}
