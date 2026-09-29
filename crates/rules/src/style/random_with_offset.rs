//! `Style/RandomWithOffset`, ported from RuboCop's
//! `lib/rubocop/cop/style/random_with_offset.rb`.
//!
//! Upstream matches three node-pattern shapes on `on_send`, restricted to
//! `RESTRICT_ON_SEND = %i[+ - succ pred next]`: `int {+ -} rand_call`
//! (`integer_op_rand?`), `rand_call {+ -} int` (`rand_op_integer?`), and
//! `rand_call.{succ pred next}` (`rand_modified?`), where `rand_call` is a
//! `rand`/`Random.rand`/`Kernel.rand`/`::Random.rand`/`::Kernel.rand` send
//! whose sole argument is an integer literal or a literal integer range.
//! [`rand_call_parts`] recovers that inner shape directly from either side
//! of the outer call (`enter` picks the receiver or the sole argument
//! depending on which side holds the integer literal), mirroring upstream's
//! shared `random_call` matcher used by all three `corrected_*` helpers.
//!
//! Every match here always yields a replacement (the arithmetic only needs
//! the literal bounds already required to match), so unlike upstream's
//! block-based `add_offense`, the whole rule is [`FixAvailability::Safe`].

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Prefer ranges when generating random numbers instead of integers with offsets.";

/// The receiver and literal bounds of one `rand`/`Random.rand`/`Kernel.rand`
/// call: `prefix` is the (possibly absent) receiver of the `rand` call
/// itself, and `left`/`right` are the inclusive bounds implied by its sole
/// argument (an integer literal or a literal integer range).
struct RandCall<'a> {
    prefix: Option<Node<'a>>,
    left: i64,
    right: i64,
}

/// Checks for the use of randomly generated numbers, added/subtracted with
/// integer literals, as well as those with `Integer#succ` and
/// `Integer#pred` methods. Prefer using ranges instead, as it clearly
/// states the intentions.
#[derive(Debug, Clone)]
pub struct RandomWithOffset;

impl Rule for RandomWithOffset {
    const META: RuleMeta = RuleMeta {
        name: "Style/RandomWithOffset",
        department: Department::Style,
        summary:
            "Prefer to use ranges when generating random numbers instead of integers with offsets.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Offsets and range endpoints are only recognized as bare integer \
            literals fitting in an `i32`, matching upstream's `def_node_matcher` patterns \
            (`(int $_)`); an offset held in a variable, produced by another expression, or \
            too large for `i32` is not flagged.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let Some(receiver) = call.receiver() else { return };
        if call.is_safe_navigation() {
            return;
        }

        let method = call.name();
        let method = method.as_slice();

        let (rand, new_left, new_right) = if method == b"+" || method == b"-" {
            let Some(args) = call.arguments() else { return };
            let list = args.arguments();
            if list.len() != 1 {
                return;
            }
            let arg = list.iter().next().expect("length checked above");
            let plus = method == b"+";

            if let Some(offset) = int_value(&receiver) {
                // `integer_op_rand?`: `int {+ -} rand_call`.
                let Some(rand) = rand_call_parts(&arg) else { return };
                let (l, r) = if plus {
                    (offset + rand.left, offset + rand.right)
                } else {
                    (offset - rand.right, offset - rand.left)
                };
                (rand, l, r)
            } else if let Some(offset) = int_value(&arg) {
                // `rand_op_integer?`: `rand_call {+ -} int`.
                let Some(rand) = rand_call_parts(&receiver) else { return };
                let (l, r) = if plus {
                    (rand.left + offset, rand.right + offset)
                } else {
                    (rand.left - offset, rand.right - offset)
                };
                (rand, l, r)
            } else {
                return;
            }
        } else if method == b"succ" || method == b"pred" || method == b"next" {
            // `rand_modified?`: `rand_call.{succ pred next}`.
            if call.arguments().is_some() {
                return;
            }
            let Some(rand) = rand_call_parts(&receiver) else { return };
            let (l, r) = if method == b"pred" {
                (rand.left - 1, rand.right - 1)
            } else {
                (rand.left + 1, rand.right + 1)
            };
            (rand, l, r)
        } else {
            return;
        };

        let span = call.as_node().span();
        let replacement = build_replacement(ctx, &rand, new_left, new_right);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement.into_bytes())],
            },
        );
    }
}

/// Mirrors upstream's shared `random_call` node matcher applied to one side
/// of the outer call: is `node` a `rand`/`Random.rand`/`Kernel.rand` send
/// (allowing a leading `::`) whose sole argument is a literal integer or
/// integer range? Returns the call's receiver (the "prefix", for
/// reconstructing e.g. `Random.rand`) and the bounds implied by that
/// argument.
fn rand_call_parts<'a>(node: &Node<'a>) -> Option<RandCall<'a>> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() {
        return None;
    }
    if call.name().as_slice() != b"rand" {
        return None;
    }
    let prefix = call.receiver();
    if !is_random_or_kernel_prefix(prefix.as_ref()) {
        return None;
    }
    let args = call.arguments()?;
    let list = args.arguments();
    if list.len() != 1 {
        return None;
    }
    let arg = list.iter().next().expect("length checked above");
    let (left, right) = literal_bounds(&arg)?;
    Some(RandCall { prefix, left, right })
}

/// Upstream's `{nil? (const {nil? cbase} :Random) (const {nil? cbase} :Kernel)}`:
/// no receiver, or a bare/top-level (`::`-prefixed) `Random` or `Kernel`
/// constant. Nested constants (e.g. `Foo::Random`) do not match, matching
/// upstream's pattern only ever matching a `const` node with a `nil`/`cbase`
/// namespace.
fn is_random_or_kernel_prefix(prefix: Option<&Node<'_>>) -> bool {
    match prefix {
        None => true,
        Some(n) => match n.kind() {
            NodeKind::ConstantReadNode => n
                .as_constant_read_node()
                .is_some_and(|c| matches!(c.name().as_slice(), b"Random" | b"Kernel")),
            NodeKind::ConstantPathNode => n.as_constant_path_node().is_some_and(|c| {
                c.parent().is_none()
                    && c.name().is_some_and(|name| matches!(name.as_slice(), b"Random" | b"Kernel"))
            }),
            _ => false,
        },
    }
}

/// Upstream's `boundaries_from_random_node`: the inclusive `(left, right)`
/// bounds implied by `rand`'s sole argument -- `0..(n - 1)` for a bare
/// integer, or the endpoints of a literal integer range (exclusive ranges
/// have their right bound decremented).
fn literal_bounds(node: &Node<'_>) -> Option<(i64, i64)> {
    match node.kind() {
        NodeKind::IntegerNode => Some((0, int_value(node)? - 1)),
        NodeKind::RangeNode => {
            let range = node.as_range_node()?;
            let left = int_value(&range.left()?)?;
            let right = int_value(&range.right()?)?;
            Some(if range.is_exclude_end() { (left, right - 1) } else { (left, right) })
        }
        _ => None,
    }
}

/// The value of an integer literal, or `None` for anything else (including
/// an integer literal too large for `i32`; see `META.blind_spots`).
fn int_value(node: &Node<'_>) -> Option<i64> {
    let int_node = node.as_integer_node()?;
    let value: i32 = int_node.value().try_into().ok()?;
    Some(i64::from(value))
}

/// Upstream's `prefix_from_prefix_node` plus the final `"#{prefix}(#{l}..#{r})"`
/// interpolation: `rand`'s own source-text receiver (if any) joined to
/// `rand`, followed by the new inclusive range literal.
fn build_replacement(
    ctx: &Context<'_>,
    rand: &RandCall<'_>,
    new_left: i64,
    new_right: i64,
) -> String {
    let prefix = match rand.prefix {
        Some(n) => format!("{}.rand", String::from_utf8_lossy(ctx.text(n.span()))),
        None => "rand".to_string(),
    };
    format!("{prefix}({new_left}..{new_right})")
}
