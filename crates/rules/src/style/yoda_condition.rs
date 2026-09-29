//! `Style/YodaCondition`, ported from RuboCop's
//! `lib/rubocop/cop/style/yoda_condition.rb`.
//!
//! Upstream's `on_send` is restricted to `RESTRICT_ON_SEND =
//! Node::COMPARISON_OPERATORS` (`== === != <= >= > <`) and never aliases
//! `on_csend`, so a safe-navigation call (`x&.== y`) is skipped entirely --
//! [`enter`] guards `!call.is_safe_navigation()` for the same reason noted
//! in the module docs this crate already carries for that trap.
//!
//! `constant_portion?` is `node.recursive_literal? || node.const_type?`;
//! [`ruby_ast::ext::is_recursive_basic_literal`] already ports
//! `recursive_literal?` (`rubocop-ast`'s `Node#recursive_basic_literal?`)
//! exactly, so this file only adds the `const_type?` half
//! ([`is_constant_portion`]).
//!
//! `interpolation?(lhs)` (`node.dstr_type? || (node.regexp_type? &&
//! node.interpolation?)`) becomes "`lhs` is Prism's
//! [`NodeKind::InterpolatedStringNode`] or
//! [`NodeKind::InterpolatedRegularExpressionNode`]" -- see `META.blind_spots`
//! for the one whitequark quirk this does not reproduce.
//!
//! `file_constant_equal_program_name?`'s node pattern (`(send
//! #source_file_path_constant? {:== :!=} (gvar #program_name?))`) becomes
//! [`is_file_constant_equal_program_name`]: Prism gives `__FILE__` its own
//! [`NodeKind::SourceFileNode`] (always sourced as exactly `__FILE__`, so no
//! text comparison is needed), and `$0`/`$PROGRAM_NAME` are ordinary
//! [`NodeKind::GlobalVariableReadNode`]s whose name bytes include the
//! leading `$`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    ForbidAll,
    ForbidEqualityOnly,
    RequireAll,
    RequireEqualityOnly,
}

impl Style {
    /// RuboCop's `enforce_yoda?`.
    fn enforce_yoda(self) -> bool {
        matches!(self, Self::RequireAll | Self::RequireEqualityOnly)
    }

    /// RuboCop's `equality_only?`.
    fn equality_only(self) -> bool {
        matches!(self, Self::ForbidEqualityOnly | Self::RequireEqualityOnly)
    }
}

/// RuboCop's `EQUALITY_OPERATORS`.
fn is_equality_operator(name: &[u8]) -> bool {
    matches!(name, b"==" | b"!=")
}

/// RuboCop's `NONCOMMUTATIVE_OPERATORS`.
fn is_noncommutative_operator(name: &[u8]) -> bool {
    name == b"==="
}

/// `RESTRICT_ON_SEND = RuboCop::AST::Node::COMPARISON_OPERATORS`.
fn is_comparison_operator(name: &[u8]) -> bool {
    matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=" | b">" | b"<")
}

/// RuboCop's `REVERSE_COMPARISON`; falls back to the operator itself (`==`,
/// `!=`, `===`).
fn reverse_comparison(operator: &[u8]) -> &[u8] {
    match operator {
        b"<" => b">",
        b"<=" => b">=",
        b">" => b"<",
        b">=" => b"<=",
        other => other,
    }
}

/// RuboCop's `constant_portion?`: `node.recursive_literal? || node.const_type?`.
fn is_constant_portion(node: &Node<'_>) -> bool {
    ext::is_recursive_basic_literal(node)
        || matches!(node.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
}

/// RuboCop's `interpolation?`: `node.dstr_type? || (node.regexp_type? &&
/// node.interpolation?)`.
fn is_interpolation(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::InterpolatedStringNode | NodeKind::InterpolatedRegularExpressionNode
    )
}

/// RuboCop's `program_name?`.
fn is_program_name(name: &[u8]) -> bool {
    matches!(name, b"$0" | b"$PROGRAM_NAME")
}

/// RuboCop's `file_constant_equal_program_name?`.
fn is_file_constant_equal_program_name(call: &CallNode<'_>) -> bool {
    if !matches!(call.name().as_slice(), b"==" | b"!=") {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    if receiver.kind() != NodeKind::SourceFileNode {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let mut arguments = args.arguments().iter();
    let Some(only) = arguments.next() else { return false };
    if arguments.next().is_some() {
        return false;
    }
    only.as_global_variable_read_node().is_some_and(|g| is_program_name(g.name().as_slice()))
}

/// RuboCop's `valid_yoda?`.
fn is_valid_yoda(call: &CallNode<'_>, style: Style) -> bool {
    let Some(args) = call.arguments() else { return true };
    let Some(rhs) = args.arguments().first() else { return true };
    let Some(lhs) = call.receiver() else { return true };

    let lhs_const = is_constant_portion(&lhs);
    let rhs_const = is_constant_portion(&rhs);
    if (lhs_const && rhs_const) || (!lhs_const && !rhs_const) || is_interpolation(&lhs) {
        return true;
    }

    if style.enforce_yoda() {
        lhs_const
    } else {
        rhs_const
    }
}

/// Forbid or enforce yoda conditions.
#[derive(Debug, Clone)]
pub struct YodaCondition {
    style: Style,
}

impl Rule for YodaCondition {
    const META: RuleMeta = RuleMeta {
        name: "Style/YodaCondition",
        department: Department::Style,
        summary: "Forbid or enforce yoda conditions.",
        explanation: "\
Enforces or forbids Yoda conditions, i.e. comparison operations where the
order of expression is reversed, e.g. `5 == x`.

```ruby
# EnforcedStyle: forbid_for_all_comparison_operators (default)
# bad
99 == foo
\"bar\" != foo
42 >= foo
10 < bar
99 == CONST

# good
foo == 99
foo == \"bar\"
foo <= 42
bar > 10
CONST == 99
\"#{interpolation}\" == foo
/#{interpolation}/ == foo
```

```ruby
# EnforcedStyle: forbid_for_equality_operators_only
# bad
99 == foo
\"bar\" != foo

# good
99 >= foo
3 < a && a < 5
```

```ruby
# EnforcedStyle: require_for_all_comparison_operators
# bad
foo == 99
foo == \"bar\"
foo <= 42
bar > 10

# good
99 == foo
\"bar\" != foo
42 >= foo
10 < bar
```

```ruby
# EnforcedStyle: require_for_equality_operators_only
# bad
99 >= foo
3 < a && a < 5

# good
99 == foo
\"bar\" != foo
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("forbid_for_all_comparison_operators"),
            allowed: &[
                "forbid_for_all_comparison_operators",
                "forbid_for_equality_operators_only",
                "require_for_all_comparison_operators",
                "require_for_equality_operators_only",
            ],
            doc: "Which comparison operators are checked and in which direction.",
        }],
        blind_spots: "\
This cop is unsafe: comparison operators can be defined differently on
different classes and are not guaranteed to have the same result if
reversed. `interpolation?(lhs)`'s `dstr_type?` half also exempts, upstream,
any multi-line plain string with no interpolation at all (whitequark
represents those as a `dstr` of per-line `str` children too); Prism parses a
multi-line literal with no interpolation as a single `StringNode`, so this
port -- unlike upstream -- can flag/require reordering a multi-line
non-interpolated string literal.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "forbid_for_equality_operators_only" => Style::ForbidEqualityOnly,
            "require_for_all_comparison_operators" => Style::RequireAll,
            "require_for_equality_operators_only" => Style::RequireEqualityOnly,
            _ => Style::ForbidAll,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() {
            return;
        }
        let name = call.name();
        let name = name.as_slice();
        if !is_comparison_operator(name) || is_noncommutative_operator(name) {
            return;
        }
        if self.style.equality_only() && !is_equality_operator(name) {
            return;
        }
        if is_file_constant_equal_program_name(&call) || is_valid_yoda(&call, self.style) {
            return;
        }

        let span = node.span();
        let source = String::from_utf8_lossy(ctx.text(span)).into_owned();
        let message = format!("Reverse the order of the operands `{source}`.");

        // `is_comparison_operator`/`is_valid_yoda` above already guarantee a
        // receiver and a first argument exist.
        let lhs = call.receiver().expect("checked in is_valid_yoda");
        let rhs = call
            .arguments()
            .expect("checked in is_valid_yoda")
            .arguments()
            .first()
            .expect("checked in is_valid_yoda");
        let lhs_src = String::from_utf8_lossy(ctx.text(lhs.span())).into_owned();
        let rhs_src = String::from_utf8_lossy(ctx.text(rhs.span())).into_owned();
        let operator = String::from_utf8_lossy(reverse_comparison(name)).into_owned();
        let corrected = format!("{rhs_src} {operator} {lhs_src}");

        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(span, corrected.into_bytes())],
        };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }
}
