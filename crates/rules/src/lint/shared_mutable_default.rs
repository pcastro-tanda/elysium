//! `Lint/SharedMutableDefault`, ported from RuboCop's
//! `lib/rubocop/cop/lint/shared_mutable_default.rb`.
//!
//! Whitequark's `hash` node type covers both brace literals and a trailing
//! bare `key: value` argument hash (differentiated only by `braces?`, not
//! node type); Prism splits the latter into `KeywordHashNode`, so
//! [`hash_like_elements`] treats both the same way everywhere upstream's
//! pattern matches `hash`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::NodeList;
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};

/// Upstream's `MSG`.
const MSG: &str = "Do not create a Hash with a mutable default value as the default value can \
                    accidentally be changed.";

/// Checks for `Hash` creation with a mutable default value.
#[derive(Debug, Clone)]
pub struct SharedMutableDefault;

impl Rule for SharedMutableDefault {
    const META: RuleMeta = RuleMeta {
        name: "Lint/SharedMutableDefault",
        department: Department::Lint,
        summary:
            "Checks for mutable literals used as default arguments during Hash initialization.",
        explanation: "\
Creating a `Hash` in such a way will share the default value
across all keys, causing unexpected behavior when modifying it.

For example, when the `Hash` was created with an `Array` as the argument,
calling `hash[:foo] << 'bar'` will also change the value of all
other keys that have not been explicitly assigned to.

```ruby
# bad
Hash.new([])
Hash.new({})
Hash.new(Array.new)
Hash.new(Hash.new)

# okay -- In rare cases that intentionally have this behavior,
#   without disabling the cop, you can set the default explicitly.
h = Hash.new
h.default = []
h[:a] << 1
h[:b] << 2
h # => {:a => [1, 2], :b => [1, 2]}

# okay -- beware this will discard mutations and only remember assignments
Hash.new { Array.new }
Hash.new { Hash.new }
Hash.new { {} }
Hash.new { [] }

# good - frozen solution will raise an error when mutation attempted
Hash.new([].freeze)
Hash.new({}.freeze)

# good - using a proc will create a new object for each key
h = Hash.new
h.default_proc = ->(h, k) { [] }
h.default_proc = ->(h, k) { {} }

# good - using a block will create a new object for each key
Hash.new { |h, k| h[k] = [] }
Hash.new { |h, k| h[k] = {} }
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"new" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if ext::const_name(&receiver).as_deref() != Some("Hash") {
            return;
        }
        let Some(args) = call.arguments() else { return };
        let args = args.arguments();
        let offends = match args.len() {
            1 => {
                let arg = args.first().expect("len checked");
                is_mutable_default_shape(&arg) && !is_capacity_keyword_argument(&arg)
            }
            2 => {
                let first = args.first().expect("len checked");
                let second = args.iter().nth(1).expect("len checked");
                is_mutable_default_shape(&first) && is_capacity_keyword_argument(&second)
            }
            _ => false,
        };
        if offends {
            ctx.report(&Self::META, node.span(), MSG);
        }
    }
}

/// Upstream's `{array hash (send (const {nil? cbase} {:Array :Hash}) :new)}`.
fn is_mutable_default_shape(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::ArrayNode | NodeKind::HashNode | NodeKind::KeywordHashNode => true,
        NodeKind::CallNode => node.as_call_node().is_some_and(|call| {
            call.name().as_slice() == b"new"
                && call.arguments().is_none()
                && call.receiver().is_some_and(|r| {
                    matches!(ext::const_name(&r).as_deref(), Some("Array" | "Hash"))
                })
        }),
        _ => false,
    }
}

/// Upstream's `capacity_keyword_argument?`: `(hash (pair (sym :capacity) _))`.
fn is_capacity_keyword_argument(node: &Node<'_>) -> bool {
    let Some(elements) = hash_like_elements(node) else { return false };
    let [only] = elements.iter().collect::<Vec<_>>()[..] else { return false };
    only.as_assoc_node().is_some_and(|assoc| {
        assoc.key().as_symbol_node().is_some_and(|sym| sym.unescaped() == b"capacity")
    })
}

/// `node`'s pairs, for either a brace `HashNode` or a trailing bare-keyword
/// `KeywordHashNode` -- both whitequark's single `hash` node type.
fn hash_like_elements<'pr>(node: &Node<'pr>) -> Option<NodeList<'pr>> {
    match node.kind() {
        NodeKind::HashNode => node.as_hash_node().map(|h| h.elements()),
        NodeKind::KeywordHashNode => node.as_keyword_hash_node().map(|h| h.elements()),
        _ => None,
    }
}
