//! `Lint/HashCompareByIdentity`, ported from RuboCop's
//! `lib/rubocop/cop/lint/hash_compare_by_identity.rb`.
//!
//! Upstream's `id_as_hash_key?` node-matcher, `(call _ {:key? :has_key?
//! :fetch :[] :[]=} (send _ :object_id) ...)`, only constrains the call's
//! *method name* (via `RESTRICT_ON_SEND`, mirrored here by [`is_hash_method`])
//! and its *first* argument: that argument must itself be a call named
//! `object_id`, on any receiver (including none, e.g. the bare `object_id`
//! in `hash.key?(object_id)`, which reads as an implicit-`self` call) and
//! with any arguments of its own (the pattern's trailing `...` after the
//! nested `(send _ :object_id)` closes that inner call, not the outer one).
//! Everything else about the call -- its own receiver, and any arguments
//! after the first -- is unconstrained, matching `[]=`'s shape where the
//! first argument is the index/key and a trailing argument is the
//! assigned value.
//!
//! Prism represents both a safe-navigation call (`hash&.key?(...)`) and a
//! plain one as the same `CallNode` kind (just a flag), so subscribing to
//! `CallNode` alone reproduces upstream's `on_send`/`alias on_csend
//! on_send` pair for free.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Use `Hash#compare_by_identity` instead of using `object_id` for keys.";

/// RuboCop's `RESTRICT_ON_SEND`.
fn is_hash_method(name: &[u8]) -> bool {
    matches!(name, b"key?" | b"has_key?" | b"fetch" | b"[]" | b"[]=")
}

/// The nested `(send _ :object_id)` half of `id_as_hash_key?`.
fn is_object_id_call(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|call| call.name().as_slice() == b"object_id")
}

/// Checks for hashes being keyed by objects' `object_id`.
#[derive(Debug, Clone, Default)]
pub struct HashCompareByIdentity;

impl HashCompareByIdentity {
    /// RuboCop's `id_as_hash_key?`.
    fn id_as_hash_key(call: &CallNode<'_>) -> bool {
        if !is_hash_method(call.name().as_slice()) {
            return false;
        }
        let Some(args) = call.arguments() else { return false };
        let Some(first) = args.arguments().first() else { return false };
        is_object_id_call(&first)
    }
}

impl Rule for HashCompareByIdentity {
    const META: RuleMeta = RuleMeta {
        name: "Lint/HashCompareByIdentity",
        department: Department::Lint,
        summary: "Checks for hashes being keyed by objects' `object_id`.",
        explanation: "\
Prefer using `Hash#compare_by_identity` rather than using `object_id` for hash keys.

This cop looks for hashes being keyed by objects' `object_id`, using one of these methods:
`key?`, `has_key?`, `fetch`, `[]` and `[]=`.

```ruby
# bad
hash = {}
hash[foo.object_id] = :bar
hash.key?(baz.object_id)

# good
hash = {}.compare_by_identity
hash[foo] = :bar
hash.key?(baz)
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
This cop is unsafe: although unlikely, the hash could store both object ids and other values that
need to be compared by value, and thus could be a false positive. It also cannot guarantee that the
receiver of one of the methods (`key?`, etc.) is actually a hash.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if Self::id_as_hash_key(&call) {
            ctx.report(&Self::META, node.span(), MSG);
        }
    }
}
