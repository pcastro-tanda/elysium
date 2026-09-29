//! `Lint/SafeNavigationConsistency`, ported from RuboCop's
//! `lib/rubocop/cop/lint/safe_navigation_consistency.rb`.
//!
//! Upstream registers `on_and`/`on_or` (an alias of the same method), which
//! the framework calls once for *every* `and`/`or` node in the file,
//! including nested ones (e.g. `a && b && c`'s inner `a && b` fires its own
//! callback). Each invocation independently walks its own operands (parens
//! block collection entirely, see [`add_operand`]), groups them by receiver
//! text, and looks for `csend`/`send` inconsistency within a group. Because
//! of that per-node re-processing, the same offense location can be found
//! more than once; RuboCop's `Base#add_offense` silently drops duplicate
//! locations reported by the same cop within one file
//! (`current_offense_locations.add?`), which [`SafeNavigationConsistency`]
//! mirrors with its own `reported` set.
//!
//! Upstream's `receiver_name_as_key` has an `if method.parent.call_type?`
//! branch that calls a `receiver` method never defined anywhere in the cop
//! (it would raise `NoMethodError` if reached). Every operand this cop
//! collects is a direct `lhs`/`rhs` of some `and`/`or` node -- parens fully
//! block `operand_nodes` from collecting anything inside them -- so that
//! branch's precondition never holds and it is not ported; the group key is
//! always the receiver's source text (or empty, for a receiver-less call).

use std::collections::HashSet;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{node::CallNode, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const USE_DOT_MSG: &str = "Use `.` instead of unnecessary `&.`.";
const USE_SAFE_NAVIGATION_MSG: &str = "Use `&.` for consistency with safe navigation.";

/// `RuboCop::AST::MethodDispatchNode::OPERATOR_METHODS`.
const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

/// `nil.methods` on Ruby 3.4 plus `NilMethods#other_stdlib_methods`
/// (`to_d`). `AllowedMethods` is merged in at [`configure`].
const NIL_METHODS: &[&[u8]] = &[
    b"!",
    b"!=",
    b"!~",
    b"&",
    b"<=>",
    b"==",
    b"===",
    b"=~",
    b"^",
    b"__id__",
    b"__send__",
    b"class",
    b"clone",
    b"define_singleton_method",
    b"display",
    b"dup",
    b"enum_for",
    b"eql?",
    b"equal?",
    b"extend",
    b"freeze",
    b"frozen?",
    b"hash",
    b"inspect",
    b"instance_eval",
    b"instance_exec",
    b"instance_of?",
    b"instance_variable_defined?",
    b"instance_variable_get",
    b"instance_variable_set",
    b"instance_variables",
    b"is_a?",
    b"itself",
    b"kind_of?",
    b"method",
    b"methods",
    b"nil?",
    b"object_id",
    b"private_methods",
    b"protected_methods",
    b"public_method",
    b"public_methods",
    b"public_send",
    b"rationalize",
    b"remove_instance_variable",
    b"respond_to?",
    b"send",
    b"singleton_class",
    b"singleton_method",
    b"singleton_methods",
    b"tap",
    b"then",
    b"to_a",
    b"to_c",
    b"to_d",
    b"to_enum",
    b"to_f",
    b"to_h",
    b"to_i",
    b"to_r",
    b"to_s",
    b"yield_self",
    b"|",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DotOp {
    /// `.`: the offending call should drop its `&.`.
    Dot,
    /// `&.`: the offending call should gain a `&.`.
    SafeNav,
}

/// One collected `and`/`or` operand: a method-dispatch node together with
/// whether its immediate parent was an `and` (`true`) or `or` (`false`)
/// node -- upstream's `operand.parent.and_type?`/`operand.parent.or_type?`.
struct Operand<'pr> {
    call: CallNode<'pr>,
    in_and: bool,
}

/// RuboCop's `collect_operands`.
fn collect_operands<'pr>(node: &Node<'pr>, out: &mut Vec<Operand<'pr>>) {
    let (left, right, in_and) = match node.kind() {
        NodeKind::AndNode => {
            let and = node.as_and_node().expect("kind matched");
            (and.left(), and.right(), true)
        }
        NodeKind::OrNode => {
            let or = node.as_or_node().expect("kind matched");
            (or.left(), or.right(), false)
        }
        _ => return,
    };
    add_operand(left, in_and, out);
    add_operand(right, in_and, out);
}

/// RuboCop's `operand_nodes`. A parenthesized (or otherwise non-`and`/`or`,
/// non-call) operand is dropped entirely -- it neither becomes an operand
/// nor is recursed into.
fn add_operand<'pr>(operand: Node<'pr>, in_and: bool, out: &mut Vec<Operand<'pr>>) {
    match operand.kind() {
        NodeKind::AndNode | NodeKind::OrNode => collect_operands(&operand, out),
        NodeKind::CallNode => {
            let call = operand.as_call_node().expect("kind matched");
            out.push(Operand { call, in_and });
        }
        _ => {}
    }
}

/// RuboCop's `receiver_name_as_key` (minus the unreachable branch, see the
/// module doc): the receiver's source text, or empty for none.
fn receiver_key(ctx: &Context<'_>, call: &CallNode<'_>) -> Vec<u8> {
    call.receiver().map_or_else(Vec::new, |r| ctx.text(r.span()).to_vec())
}

/// RuboCop's `MethodDispatchNode#operator_method?`.
fn is_operator_method(name: &[u8]) -> bool {
    OPERATOR_METHODS.contains(&name)
}

/// RuboCop's `MethodDispatchNode#dot?`.
fn is_dot_call(call: &CallNode<'_>) -> bool {
    !call.is_safe_navigation() && call.call_operator_loc().is_some()
}

/// RuboCop's `find_consistent_parts`, given `most_left_indices`' four
/// group-local indices. Returns the expected dot operator and the index of
/// the first operand that must match it.
fn find_consistent_parts(
    csend_in_and: Option<usize>,
    csend_in_or: Option<usize>,
    send_in_and: Option<usize>,
    send_in_or: Option<usize>,
) -> Option<(DotOp, usize)> {
    if let (Some(cia), Some(cio)) = (csend_in_and, csend_in_or) {
        if cia < cio {
            return None;
        }
    }

    if let Some(cia) = csend_in_and {
        let begin = send_in_and.map_or(cia, |sia| sia.min(cia)) + 1;
        return Some((DotOp::Dot, begin));
    }

    if let (Some(sio), Some(cio)) = (send_in_or, csend_in_or) {
        return Some(if sio < cio { (DotOp::Dot, sio + 1) } else { (DotOp::SafeNav, cio + 1) });
    }

    if let (Some(sia), Some(cio)) = (send_in_and, csend_in_or) {
        if sia < cio {
            return Some((DotOp::Dot, cio));
        }
    }

    None
}

/// RuboCop's `already_appropriate_call?`.
fn already_appropriate_call(call: &CallNode<'_>, dot_op: DotOp) -> bool {
    if call.is_safe_navigation() && dot_op == DotOp::SafeNav {
        return true;
    }
    (is_dot_call(call) || is_operator_method(call.name().as_slice())) && dot_op == DotOp::Dot
}

/// Check to make sure that if safe navigation is used in an `&&` or `||`
/// condition, consistent and appropriate safe navigation, without excess or
/// deficiency, is used for all method calls on the same object.
#[derive(Debug, Clone)]
pub struct SafeNavigationConsistency {
    /// `NilMethods#nil_methods`: `nil.methods + other_stdlib_methods +
    /// allowed_methods`.
    nilable_methods: HashSet<Vec<u8>>,
    /// Offense locations already reported in this file, mirroring
    /// `Base#current_offense_locations`.
    reported: HashSet<Span>,
}

impl SafeNavigationConsistency {
    /// RuboCop's `nilable?`.
    fn is_nilable(&self, call: &CallNode<'_>) -> bool {
        call.is_safe_navigation() || self.nilable_methods.contains(call.name().as_slice())
    }

    /// RuboCop's `most_left_indices` plus `find_consistent_parts` plus the
    /// `next unless (dot_op, begin_of_rest_operands = ...)` / rest-operands
    /// loop, scoped to one receiver group.
    fn process_group(&mut self, operands: &[Operand<'_>], group: &[usize], ctx: &mut Context<'_>) {
        let mut csend_in_and = None;
        let mut csend_in_or = None;
        let mut send_in_and = None;
        let mut send_in_or = None;

        for (local, &global) in group.iter().enumerate() {
            let call = &operands[global].call;
            let is_csend = call.is_safe_navigation();
            let nilable = self.is_nilable(call);
            if operands[global].in_and {
                if is_csend && csend_in_and.is_none() {
                    csend_in_and = Some(local);
                }
                if !nilable && send_in_and.is_none() {
                    send_in_and = Some(local);
                }
            } else {
                if is_csend && csend_in_or.is_none() {
                    csend_in_or = Some(local);
                }
                if !nilable && send_in_or.is_none() {
                    send_in_or = Some(local);
                }
            }
        }

        let Some((dot_op, begin)) =
            find_consistent_parts(csend_in_and, csend_in_or, send_in_and, send_in_or)
        else {
            return;
        };
        let Some(rest) = group.get(begin..) else { return };

        for &global in rest {
            let call = operands[global].call;
            if already_appropriate_call(&call, dot_op) {
                continue;
            }
            self.register_offense(&call, dot_op, ctx);
        }
    }

    /// RuboCop's `register_offense`.
    fn register_offense(&mut self, call: &CallNode<'_>, dot_op: DotOp, ctx: &mut Context<'_>) {
        let is_operator = is_operator_method(call.name().as_slice());
        let operator_loc = call.call_operator_loc();
        let span = if is_operator {
            call.as_node().span()
        } else {
            operator_loc.as_ref().map_or_else(|| call.as_node().span(), ruby_ast::LocationExt::span)
        };
        if !self.reported.insert(span) {
            return;
        }

        let message = match dot_op {
            DotOp::Dot => USE_DOT_MSG,
            DotOp::SafeNav => USE_SAFE_NAVIGATION_MSG,
        };

        if is_operator {
            ctx.report(&Self::META, span, message);
            return;
        }
        let Some(operator_loc) = operator_loc else {
            ctx.report(&Self::META, span, message);
            return;
        };
        let replacement: &[u8] = match dot_op {
            DotOp::Dot => b".",
            DotOp::SafeNav => b"&.",
        };
        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(operator_loc.span(), replacement.to_vec())],
        };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }
}

impl Rule for SafeNavigationConsistency {
    const META: RuleMeta = RuleMeta {
        name: "Lint/SafeNavigationConsistency",
        department: Department::Lint,
        summary: "Check to make sure that if safe navigation is used in an `&&` or `||` condition, consistent and appropriate safe navigation, without excess or deficiency, is used for all method calls on the same object.",
        explanation: "Checks that if safe navigation is used in an `&&` or `||` condition, \
consistent and appropriate safe navigation, without excess or deficiency, is used for all \
method calls on the same object.

# Examples

```ruby
# bad
foo&.bar && foo&.baz

# good
foo&.bar && foo.baz

# bad
foo.bar && foo&.baz

# good
foo.bar && foo.baz

# bad
foo&.bar || foo.baz

# good
foo&.bar || foo&.baz

# bad
foo.bar || foo&.baz

# good
foo.bar || foo.baz

# bad
foo&.bar && (foobar.baz || foo&.baz)

# good
foo&.bar && (foobar.baz || foo.baz)
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::AndNode, NodeKind::OrNode],
        config: &[ConfigOption {
            name: "AllowedMethods",
            default: ConfigDefault::StrList(&["present?", "blank?", "presence", "try", "try!"]),
            allowed: &[],
            doc: "Allowed methods that will be always accepted regardless of whether they're \
inconsistent with the rest of a group of calls to the same object.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let mut nilable_methods: HashSet<Vec<u8>> =
            NIL_METHODS.iter().map(|m| m.to_vec()).collect();
        for allowed in options.str_list("AllowedMethods") {
            nilable_methods.insert(allowed.into_bytes());
        }
        Ok(Self { nilable_methods, reported: HashSet::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let mut operands = Vec::new();
        collect_operands(node, &mut operands);
        if operands.len() < 2 {
            return;
        }

        let mut groups: Vec<(Vec<u8>, Vec<usize>)> = Vec::new();
        for (idx, operand) in operands.iter().enumerate() {
            let key = receiver_key(ctx, &operand.call);
            match groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, indices)) => indices.push(idx),
                None => groups.push((key, vec![idx])),
            }
        }

        for (_, indices) in &groups {
            self.process_group(&operands, indices, ctx);
        }
    }
}
