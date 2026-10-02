//! `Lint/DataDefineOverride`, ported from RuboCop's
//! `lib/rubocop/cop/lint/data_define_override.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// Upstream's `DATA_METHOD_NAMES`: `Data.define.instance_methods.sort` in Ruby 4.0.0.
const DATA_METHOD_NAMES: &[&[u8]] = &[
    b"!",
    b"!=",
    b"!~",
    b"<=>",
    b"==",
    b"===",
    b"__id__",
    b"__send__",
    b"class",
    b"clone",
    b"deconstruct",
    b"deconstruct_keys",
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
    b"members",
    b"method",
    b"methods",
    b"nil?",
    b"object_id",
    b"private_methods",
    b"protected_methods",
    b"public_method",
    b"public_methods",
    b"public_send",
    b"remove_instance_variable",
    b"respond_to?",
    b"send",
    b"singleton_class",
    b"singleton_method",
    b"singleton_methods",
    b"tap",
    b"then",
    b"to_enum",
    b"to_h",
    b"to_s",
    b"with",
    b"yield_self",
];

/// `(send (const {nil? cbase} :Data) :define ...)`.
fn is_data_define(call: &ruby_ast::node::CallNode<'_>) -> bool {
    if call.name().as_slice() != b"define" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    match receiver.kind() {
        NodeKind::ConstantReadNode => {
            receiver.as_constant_read_node().is_some_and(|n| n.name().as_slice() == b"Data")
        }
        NodeKind::ConstantPathNode => {
            let Some(n) = receiver.as_constant_path_node() else { return false };
            n.parent().is_none() && n.name().is_some_and(|id| id.as_slice() == b"Data")
        }
        _ => false,
    }
}

/// Checks unexpected overrides of the `Data` built-in methods via `Data.define`.
#[derive(Debug, Clone)]
pub struct DataDefineOverride;

impl Rule for DataDefineOverride {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DataDefineOverride",
        department: Department::Lint,
        summary: "Disallow overriding the `Data` built-in methods via `Data.define`.",
        explanation: "\
Checks unexpected overrides of the `Data` built-in methods via `Data.define`.

```ruby
# bad
Bad = Data.define(:members, :clone, :to_s)
b = Bad.new(members: [], clone: true, to_s: 'bad')
b.members #=> [] (overriding `Data#members`)
b.clone #=> true (overriding `Object#clone`)
b.to_s #=> \"bad\" (overriding `Data#to_s`)

# good
Good = Data.define(:id, :name)
g = Good.new(id: 1, name: \"foo\")
g.members #=> [:id, :name]
g.clone #=> #<data Good id=1, name=\"foo\">
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
        if !is_data_define(&call) {
            return;
        }
        let Some(args) = call.arguments() else { return };
        for arg in &args.arguments() {
            let member_name = match arg.kind() {
                NodeKind::SymbolNode => {
                    let Some(n) = arg.as_symbol_node() else { continue };
                    (format!(":{}", String::from_utf8_lossy(n.unescaped())), n.unescaped().to_vec())
                }
                NodeKind::StringNode => {
                    let Some(n) = arg.as_string_node() else { continue };
                    (
                        format!("\"{}\"", String::from_utf8_lossy(n.unescaped())),
                        n.unescaped().to_vec(),
                    )
                }
                _ => continue,
            };
            let (inspected, bytes) = member_name;
            if !DATA_METHOD_NAMES.contains(&bytes.as_slice()) {
                continue;
            }
            let method_name = String::from_utf8_lossy(&bytes);
            let message = format!(
                "`{inspected}` member overrides `Data#{method_name}` and it may be unexpected."
            );
            ctx.report(&Self::META, arg.span(), message);
        }
    }
}
