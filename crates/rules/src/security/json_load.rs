//! `Security/JSONLoad`, ported from RuboCop's
//! `lib/rubocop/cop/security/json_load.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{ext, LocationExt as _, Node, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Prefer `JSON.parse` over `JSON.{}`.";

/// Checks for the use of `JSON` class methods which have potential
/// security issues.
#[derive(Debug, Clone)]
pub struct JSONLoad;

impl Rule for JSONLoad {
    const META: RuleMeta = RuleMeta {
        name: "Security/JSONLoad",
        department: Department::Security,
        summary: "Prefer usage of `JSON.parse` over `JSON.load` due to potential security issues. See reference for more information.",
        explanation: "\
Checks for the use of JSON class methods which have potential
security issues.

`JSON.load` and similar methods allow deserialization of arbitrary ruby objects:

```ruby
require 'json/add/string'
result = JSON.load('{ \"json_class\": \"String\", \"raw\": [72, 101, 108, 108, 111] }')
pp result # => \"Hello\"
```

Never use `JSON.load` for untrusted user input. Prefer `JSON.parse` unless you have
a concrete use-case for `JSON.load`.

```ruby
# bad
JSON.load('{}')
JSON.restore('{}')

# good
JSON.parse('{}')
JSON.unsafe_load('{}')

# good - explicit use of `create_additions` option
JSON.load('{}', create_additions: true)
JSON.load('{}', create_additions: false)
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
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
        let Some(method) = insecure_json_load(&call) else { return };
        let Some(selector) = call.message_loc() else { return };
        let span = selector.span();
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG.replacen("{}", method, 1),
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, b"parse".to_vec())],
            },
        );
    }
}

/// RuboCop's `insecure_json_load` node-matcher, `(send (const {nil? cbase}
/// :JSON) ${:load :restore} ... !(hash <descendant (sym $:create_additions)>))`. A
/// bare-or-toplevel-qualified `JSON.load`/`JSON.restore` call with at
/// least one argument, whose last argument is not a hash (or keyword-hash)
/// containing a `create_additions:` key.
fn insecure_json_load(call: &CallNode<'_>) -> Option<&'static str> {
    let method = match call.name().as_slice() {
        b"load" => "load",
        b"restore" => "restore",
        _ => return None,
    };
    let receiver = call.receiver()?;
    if !is_json_const(&receiver) {
        return None;
    }
    let arguments = call.arguments()?;
    let arg_list = arguments.arguments();
    let last = arg_list.last()?;
    if has_create_additions_key(&last) {
        return None;
    }
    Some(method)
}

/// `(const {nil? cbase} :JSON)`: a bare or top-level-qualified `JSON`
/// constant reference.
fn is_json_const(node: &Node<'_>) -> bool {
    ext::is_bare_or_toplevel_const(node) && ext::const_name(node).is_some_and(|name| name == "JSON")
}

/// Whether `node` is a hash (braced `HashNode` or braceless
/// `KeywordHashNode`) with an entry keyed by the symbol
/// `:create_additions`, mirroring the upstream pattern's recursive search
/// for a `(sym :create_additions)` descendant inside the hash.
fn has_create_additions_key(node: &Node<'_>) -> bool {
    let pairs: Vec<Node<'_>> = if let Some(hash) = node.as_hash_node() {
        hash.elements().iter().collect()
    } else if let Some(hash) = node.as_keyword_hash_node() {
        hash.elements().iter().collect()
    } else {
        return false;
    };
    pairs.iter().any(|pair| {
        pair.as_assoc_node().is_some_and(|assoc| is_create_additions_symbol(&assoc.key()))
    })
}

/// Whether `node` is the symbol literal `:create_additions`.
fn is_create_additions_symbol(node: &Node<'_>) -> bool {
    node.as_symbol_node().is_some_and(|sym| sym.unescaped() == b"create_additions")
}
