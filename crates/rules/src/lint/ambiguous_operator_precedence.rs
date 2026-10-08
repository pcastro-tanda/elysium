//! `Lint/AmbiguousOperatorPrecedence`, ported from RuboCop's
//! `lib/rubocop/cop/lint/ambiguous_operator_precedence.rb`.
//!
//! Prism has no parent pointers, so [`Analysis`] does one recursive descent
//! from `file_start`, building a `(kind, span) -> parent` map plus the list
//! of every `AndNode` and every binary-operator `CallNode` in the file --
//! `(kind, span)` is a stable identity per `Lint/RedundantSafeNavigation`'s
//! own doc comment, since Prism never hands out two distinct nodes sharing
//! both.
//!
//! Upstream dispatches `on_and` (fires for both `&&` and the `and` keyword,
//! whitequark's single `:and` type) and `on_send` (restricted to
//! `RESTRICT_ON_SEND`, upstream's `PRECEDENCE.flatten`) separately, with
//! different logic, and this port keeps them separate rather than folding
//! `on_and`'s structural check into the generic precedence comparison: the
//! `and`/`or` *keyword* forms always bind looser than `&&`/`||`, which means
//! an `AndNode` can be the *textual* `&&` yet still sit as the direct child
//! of an `or`-keyword `OrNode` (`a && b or c`); upstream's `on_and` flags
//! that (it only checks `parent.or_type?`, never the parent's own operator
//! text), while the generic precedence table has no entry for `:or` (only
//! `:||`), so folding the two would silently drop that case.
//!
//! Prism unifies `&&`/`and` into one `AndNode` kind (`operator_loc` holds
//! whichever text was used) and `||`/`or` into one `OrNode` kind, matching
//! whitequark's own unification -- so no extra dispatch is needed there
//! either.

use std::collections::HashMap;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{AndNode, CallNode};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Wrap expressions with varying precedence with parentheses to avoid ambiguity.";

/// Upstream's `PRECEDENCE` table, loosest-binding last.
const PRECEDENCE: &[&[&[u8]]] = &[
    &[b"**"],
    &[b"*", b"/", b"%"],
    &[b"+", b"-"],
    &[b"<<", b">>"],
    &[b"&"],
    &[b"|", b"^"],
    &[b"&&"],
    &[b"||"],
];

fn precedence_of(operator: &[u8]) -> Option<usize> {
    PRECEDENCE.iter().position(|group| group.contains(&operator))
}

/// Upstream's `operator_name`: a `CallNode`'s method name, or an
/// `AndNode`/`OrNode`'s actual operator text (`&&`/`and`, `||`/`or`).
fn operator_precedence(node: &Node<'_>, ctx: &Context<'_>) -> Option<usize> {
    let bytes: &[u8] = match node.kind() {
        NodeKind::CallNode => node.as_call_node()?.name().as_slice(),
        NodeKind::AndNode => ctx.text(node.as_and_node()?.operator_loc().span()),
        NodeKind::OrNode => ctx.text(node.as_or_node()?.operator_loc().span()),
        _ => return None,
    };
    precedence_of(bytes)
}

/// Upstream's `operator?`: a binary-operator `CallNode`, or any
/// `AndNode`/`OrNode` (`operator_keyword?`, true regardless of the actual
/// `&&`/`and`/`||`/`or` text used).
fn is_operator_node(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::CallNode => {
            node.as_call_node().is_some_and(|c| precedence_of(c.name().as_slice()).is_some())
        }
        NodeKind::AndNode | NodeKind::OrNode => true,
        _ => false,
    }
}

/// Checks for expressions containing multiple binary operations where precedence is ambiguous due to lack of parentheses.
#[derive(Debug, Clone)]
pub struct AmbiguousOperatorPrecedence;

impl Rule for AmbiguousOperatorPrecedence {
    const META: RuleMeta = RuleMeta {
        name: "Lint/AmbiguousOperatorPrecedence",
        department: Department::Lint,
        summary: "Checks for expressions containing multiple binary operations with ambiguous precedence.",
        explanation: "\
Looks for expressions containing multiple binary operators where precedence \
is ambiguous due to lack of parentheses. For example, in `1 + 2 * 3`, the \
multiplication will happen before the addition, but lexically it appears \
that the addition will happen first.

The cop does not consider unary operators (ie. `!a` or `-b`) or comparison \
operators (ie. `a =~ b`) because those are not ambiguous.

NOTE: Ranges are handled by `Lint/AmbiguousRange`.

```ruby
# bad
a + b * c
a || b && c
a ** b + c

# good (different precedence)
a + (b * c)
a || (b && c)
(a ** b) + c

# good (same precedence)
a + b + c
a * b / c % d
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let analysis = Analysis::build(root);

        let mut offenses: Vec<Span> = Vec::new();

        for and_node in &analysis.and_nodes {
            let node = and_node.as_node();
            if analysis.parent_of(&node).is_some_and(|p| p.kind() == NodeKind::OrNode) {
                offenses.push(node.span());
            }
        }

        for call in &analysis.operator_calls {
            let node = call.as_node();
            if call.opening_loc().is_some() {
                continue;
            }
            let Some(parent) = analysis.parent_of(&node) else { continue };
            if !is_operator_node(&parent) {
                continue;
            }
            let Some(node_precedence) = operator_precedence(&node, ctx) else { continue };
            let Some(parent_precedence) = operator_precedence(&parent, ctx) else { continue };
            if parent_precedence > node_precedence {
                offenses.push(node.span());
            }
        }

        offenses.sort_by_key(|span| span.start);
        for span in offenses {
            let fix = Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::insert(span.start, b"(".to_vec()),
                    Edit::insert(span.end, b")".to_vec()),
                ],
            };
            ctx.report_with_fix(&Self::META, span, MSG, fix);
        }
    }
}

/// `(kind, span) -> parent` map plus the list of every `AndNode` and every
/// binary-operator `CallNode` in the file, built by one recursive descent
/// from the root.
struct Analysis<'pr> {
    parent: HashMap<(NodeKind, Span), Node<'pr>>,
    and_nodes: Vec<AndNode<'pr>>,
    operator_calls: Vec<CallNode<'pr>>,
}

impl<'pr> Analysis<'pr> {
    fn build(root: Node<'pr>) -> Self {
        let mut analysis =
            Self { parent: HashMap::new(), and_nodes: Vec::new(), operator_calls: Vec::new() };
        analysis.visit(root);
        analysis
    }

    /// `Node#each_child_node` yields a `CallNode`'s own arguments directly
    /// as children (no wrapper), whitequark's `send` node shape; Prism
    /// interposes an `ArgumentsNode` holding them. So the logical parent
    /// recorded for an `ArgumentsNode`'s own children skips through to
    /// *its* parent (the call), matching the whitequark shape this cop's
    /// logic (ported from `on_send`'s `node.parent`) assumes.
    fn visit(&mut self, node: Node<'pr>) {
        if let Some(and_node) = node.as_and_node() {
            self.and_nodes.push(and_node);
        }
        if let Some(call) = node.as_call_node() {
            if precedence_of(call.name().as_slice()).is_some() {
                self.operator_calls.push(call);
            }
        }
        let logical_parent = if node.kind() == NodeKind::ArgumentsNode {
            self.parent_of(&node).unwrap_or(node)
        } else {
            node
        };
        let mut children = Vec::new();
        for_each_child(&node, |child| children.push(*child));
        for child in children {
            self.parent.insert((child.kind(), child.span()), logical_parent);
            self.visit(child);
        }
    }

    fn parent_of(&self, node: &Node<'pr>) -> Option<Node<'pr>> {
        self.parent.get(&(node.kind(), node.span())).copied()
    }
}
