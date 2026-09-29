//! `Style/ZeroLengthPredicate`, ported from RuboCop's
//! `lib/rubocop/cop/style/zero_length_predicate.rb`.
//!
//! Upstream dispatches through `RESTRICT_ON_SEND = %i[size length]`: the
//! `on_send`/`on_csend` hooks fire on the *inner* `.size`/`.length` call and
//! then pattern-match its parent. This port instead walks every
//! [`NodeKind::CallNode`] and matches the *outer* shape directly (the
//! `.zero?` predicate call, or the comparison operator call) against its
//! receiver/argument, since Prism gives no cheap way to ask "was I just
//! entered as someone's parent". [`match_predicate`] and
//! [`match_comparison`] together cover exactly the same eight shapes
//! upstream's four `def_node_matcher`s do.
//!
//! # `csend` gating
//!
//! Upstream registers `check_zero_length_predicate` and
//! `check_zero_length_comparison` under both `on_send` and `on_csend`, but
//! `check_nonzero_length_comparison` only under `on_send`. Since `RESTRICT_ON_SEND`
//! keys off the *inner* `.size`/`.length` call's own dispatch (`send` vs
//! `csend`), a nonzero comparison (`>`, `!=`, or the reversed `<`/`!=`) is
//! only ever flagged when that inner call is a plain `.` -- never `&.`. The
//! outer comparison/predicate call's own send-vs-csend-ness never matters
//! (upstream's patterns use the type-agnostic `call` node type for it), so
//! [`match_comparison`] gates only on `inner.is_safe_navigation()`.
//!
//! # Message text: raw source vs. synthesized
//!
//! The predicate case's message embeds `node.loc.selector.join(parent...)`'s
//! *raw source* (`length.zero?`, `length&.zero?`, ...), so [`Rule::enter`]
//! reads it straight out of the file. The comparison case's message instead
//! synthesizes `"#{method_name} #{op} #{int_literal}"` from the matched
//! symbols/literal (`length == 0`), never the receiver's own source --
//! [`match_comparison`] builds that string directly.
//!
//! # Non-polymorphic collections
//!
//! `non_polymorphic_collection?` matches the *outer* node.parent's
//! three-level chain (`outer.middle.inner`), which -- for either matched
//! shape here -- reduces to asking whether the `.size`/`.length` call's own
//! receiver is itself a `File.stat(...)`/`File.new(...)`/`Tempfile.new(...)`/
//! `StringIO.new(...)`/`File::Stat.new(...)` call; [`is_non_polymorphic`]
//! checks exactly that, using [`const_name`] to collapse a leading `::`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::const_name;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `RESTRICT_ON_SEND`'s method names, mirrored as a receiver-presence,
/// no-argument shape: rubocop-ast's `(call (...) {:length :size})`.
fn length_call<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    if call.receiver().is_none() || call.arguments().is_some() {
        return None;
    }
    matches!(call.name().as_slice(), b"size" | b"length").then_some(call)
}

/// The sole positional argument of a call, if it has exactly one.
fn only_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let args = call.arguments()?;
    let list = args.arguments();
    (list.len() == 1).then(|| list.iter().next()).flatten()
}

/// An `(int N)` literal's value.
fn int_value(node: &Node<'_>) -> Option<i32> {
    node.as_integer_node().and_then(|n| TryInto::<i32>::try_into(n.value()).ok())
}

/// `non_polymorphic_collection?`: is `inner`'s own receiver a constructor
/// call from the standard-library `File`/`Tempfile`/`StringIO` family whose
/// `#size` is not backed by `#empty?`?
fn is_non_polymorphic(inner: &CallNode<'_>) -> bool {
    let Some(ctor) = inner.receiver().and_then(|r| r.as_call_node()) else { return false };
    let Some(receiver) = ctor.receiver() else { return false };
    let name = const_name(&receiver);
    matches!(
        (ctor.name().as_slice(), name.as_deref()),
        (b"stat", Some("File"))
            | (b"new" | b"open", Some("File" | "Tempfile" | "StringIO"))
            | (b"new", Some("File::Stat"))
    )
}

/// One matched offense shape.
enum Matched<'pr> {
    /// `x.length.zero?` -- offense from the inner call's selector through
    /// the outer call's end, replaced with a literal `empty?`.
    Predicate { inner: CallNode<'pr> },
    /// A `==`/`<`/`>`/`!=` comparison against `0`/`1` -- offense is the
    /// whole outer call, replaced with `receiver.empty?` (or its negation).
    Comparison { inner: CallNode<'pr>, nonzero: bool, display: String },
}

/// `zero_length_predicate?`: `(call (call (...) {:length :size}) :zero?)`.
fn match_predicate<'pr>(call: &CallNode<'pr>) -> Option<Matched<'pr>> {
    if call.name().as_slice() != b"zero?" || call.arguments().is_some() {
        return None;
    }
    let inner = length_call(&call.receiver()?)?;
    Some(Matched::Predicate { inner })
}

/// `zero_length_comparison`/`nonzero_length_comparison` together: every
/// `{==, <, >, !=}` comparison of a `.size`/`.length` call against `0`/`1`,
/// in either operand order. Nonzero shapes are rejected when `inner` is a
/// safe-navigation call (see the module doc's `csend` gating note).
fn match_comparison<'pr>(call: &CallNode<'pr>) -> Option<Matched<'pr>> {
    let op = call.name();
    let op = op.as_slice();
    if !matches!(op, b"==" | b"<" | b">" | b"!=") {
        return None;
    }
    let receiver = call.receiver()?;
    let arg = only_argument(call)?;

    // `x.length OP int` -- the length/size call is the receiver.
    let (inner, nonzero, display) = if let Some(inner) = length_call(&receiver) {
        let n = int_value(&arg)?;
        let name = String::from_utf8_lossy(inner.name().as_slice());
        match (op, n) {
            (b"==", 0) => (inner, false, format!("{name} == 0")),
            (b"<", 1) => (inner, false, format!("{name} < 1")),
            (b">", 0) => (inner, true, format!("{name} > 0")),
            (b"!=", 0) => (inner, true, format!("{name} != 0")),
            _ => return None,
        }
    // `int OP x.length` -- the length/size call is the sole argument.
    } else {
        let inner = length_call(&arg)?;
        let n = int_value(&receiver)?;
        let name = String::from_utf8_lossy(inner.name().as_slice());
        match (op, n) {
            (b"==", 0) => (inner, false, format!("0 == {name}")),
            (b">", 1) => (inner, false, format!("1 > {name}")),
            (b"<", 0) => (inner, true, format!("0 < {name}")),
            (b"!=", 0) => (inner, true, format!("0 != {name}")),
            _ => return None,
        }
    };

    if nonzero && inner.is_safe_navigation() {
        return None;
    }
    Some(Matched::Comparison { inner, nonzero, display })
}

/// Checks for numeric comparisons that can be replaced by a predicate
/// method, such as `receiver.length == 0`, `receiver.length > 0`, and
/// `receiver.length != 0`, `receiver.length < 1` and `receiver.size == 0`
/// that can be replaced by `receiver.empty?` and `!receiver.empty?`.
#[derive(Debug, Clone)]
pub struct ZeroLengthPredicate;

impl Rule for ZeroLengthPredicate {
    const META: RuleMeta = RuleMeta {
        name: "Style/ZeroLengthPredicate",
        department: Department::Style,
        summary: "Use #empty? when testing for objects of length 0.",
        explanation: "\
Checks for numeric comparisons that can be replaced by a predicate method,
such as `receiver.length == 0`, `receiver.length > 0`, and
`receiver.length != 0`, `receiver.length < 1` and `receiver.size == 0` that
can be replaced by `receiver.empty?` and `!receiver.empty?`.

`File`, `Tempfile`, `StringIO`, and `File::Stat` do not have `empty?` so
this cop allows `size == 0` and `size.zero?` for them. When a `File::Stat`
object is stored in a variable, the cop cannot detect the type and may still
register a false positive.

@safety
  This cop is unsafe because it cannot be guaranteed that the receiver has
  an `empty?` method that is defined in terms of `length`. If there is a
  non-standard class that redefines `length` or `empty?`, the cop may
  register a false positive.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
`non_polymorphic_collection?` only recognizes the `File.stat(...)`/
`File.new(...)`/`Tempfile.new(...)`/`StringIO.new(...)`/`File::Stat.new(...)`
call shapes textually; a `File::Stat` (etc.) instance reached through a
variable or another method call is not recognized and may false-positive,
matching upstream's own documented limitation.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let Some(matched) = match_predicate(&call).or_else(|| match_comparison(&call)) else {
            return;
        };

        let inner = match &matched {
            Matched::Predicate { inner } | Matched::Comparison { inner, .. } => inner,
        };
        if is_non_polymorphic(inner) {
            return;
        }

        match matched {
            Matched::Predicate { inner } => {
                let start =
                    inner.message_loc().map_or(inner.as_node().span().start, |l| l.span().start);
                let span = Span::new(start, node.span().end);
                let current = String::from_utf8_lossy(ctx.text(span)).into_owned();
                let message = format!("Use `empty?` instead of `{current}`.");
                ctx.report_with_fix(
                    &Self::META,
                    span,
                    message,
                    Fix {
                        applicability: Applicability::Unsafe,
                        edits: vec![Edit::replace(span, b"empty?".to_vec())],
                    },
                );
            }
            Matched::Comparison { inner, nonzero, display } => {
                let span = node.span();
                let dot = inner.call_operator_loc().map_or(&b"."[..], |l| ctx.text(l.span()));
                let receiver_text =
                    ctx.text(inner.receiver().expect("length_call requires a receiver").span());
                let mut replacement = Vec::with_capacity(receiver_text.len() + dot.len() + 7);
                if nonzero {
                    replacement.push(b'!');
                }
                replacement.extend_from_slice(receiver_text);
                replacement.extend_from_slice(dot);
                replacement.extend_from_slice(b"empty?");

                let prefix = if nonzero { "!" } else { "" };
                let message = format!("Use `{prefix}empty?` instead of `{display}`.");
                ctx.report_with_fix(
                    &Self::META,
                    span,
                    message,
                    Fix {
                        applicability: Applicability::Unsafe,
                        edits: vec![Edit::replace(span, replacement)],
                    },
                );
            }
        }
    }
}
