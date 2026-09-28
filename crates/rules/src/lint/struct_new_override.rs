//! `Lint/StructNewOverride`, ported from RuboCop's
//! `lib/rubocop/cop/lint/struct_new_override.rb`.
//!
//! # `STRUCT_METHOD_NAMES`
//!
//! Upstream computes this list once, at load time, by calling
//! `Struct.instance_methods` on whatever Ruby the RuboCop process itself
//! runs on. This port hardcodes the result of that call on Ruby 3.4.2 (the
//! interpreter this repository's fixtures are generated against; see
//! `tools/port_spec.rb`) as [`STRUCT_METHOD_NAMES`] instead of reproducing
//! Ruby's method-resolution machinery at run time. A member name that is a
//! `Struct`/`Enumerable`/`Object`/`Kernel` instance method on a *different*
//! Ruby version (one that has added or removed such a method) would be
//! mis-classified; not exercised by any fixture.
//!
//! # Argument shapes
//!
//! `STRUCT_MEMBER_NAME_TYPES` is `%i[sym str]`: only a plain literal symbol
//! or string argument is ever treated as a member name. Prism's equivalents
//! are `SymbolNode` and `StringNode`; an interpolated symbol/string
//! (`InterpolatedSymbolNode`/`InterpolatedStringNode`) is a different node
//! kind and so is never considered, matching upstream (whitequark's `dsym`/
//! `dstr` are likewise excluded from `%i[sym str]`). A `keyword_init: true`
//! trailing option is Prism's `KeywordHashNode`, which is neither kind and
//! so is skipped without any special-casing, exactly as upstream's type
//! check skips its whitequark `hash` node for free.
//!
//! The first argument is exempt only when it is itself a plain `str` (the
//! optional class-name argument, `Struct.new('Name', :a, :b)`); a first
//! argument that is a `sym` is still a member name (`Struct.new(:members)`
//! offends), matching upstream's `index.zero? && arg.str_type?` guard.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `STRUCT_METHOD_NAMES`: `Struct.instance_methods` on Ruby
/// 3.4.2. See the module doc.
const STRUCT_METHOD_NAMES: &[&[u8]] = &[
    b"!",
    b"!=",
    b"!~",
    b"<=>",
    b"==",
    b"===",
    b"[]",
    b"[]=",
    b"__id__",
    b"__send__",
    b"all?",
    b"any?",
    b"chain",
    b"chunk",
    b"chunk_while",
    b"class",
    b"clone",
    b"collect",
    b"collect_concat",
    b"compact",
    b"count",
    b"cycle",
    b"deconstruct",
    b"deconstruct_keys",
    b"define_singleton_method",
    b"detect",
    b"dig",
    b"display",
    b"drop",
    b"drop_while",
    b"dup",
    b"each",
    b"each_cons",
    b"each_entry",
    b"each_pair",
    b"each_slice",
    b"each_with_index",
    b"each_with_object",
    b"entries",
    b"enum_for",
    b"eql?",
    b"equal?",
    b"extend",
    b"filter",
    b"filter_map",
    b"find",
    b"find_all",
    b"find_index",
    b"first",
    b"flat_map",
    b"freeze",
    b"frozen?",
    b"grep",
    b"grep_v",
    b"group_by",
    b"hash",
    b"include?",
    b"inject",
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
    b"lazy",
    b"length",
    b"map",
    b"max",
    b"max_by",
    b"member?",
    b"members",
    b"method",
    b"methods",
    b"min",
    b"min_by",
    b"minmax",
    b"minmax_by",
    b"nil?",
    b"none?",
    b"object_id",
    b"one?",
    b"partition",
    b"private_methods",
    b"protected_methods",
    b"public_method",
    b"public_methods",
    b"public_send",
    b"reduce",
    b"reject",
    b"remove_instance_variable",
    b"respond_to?",
    b"reverse_each",
    b"select",
    b"send",
    b"singleton_class",
    b"singleton_method",
    b"singleton_methods",
    b"size",
    b"slice_after",
    b"slice_before",
    b"slice_when",
    b"sort",
    b"sort_by",
    b"sum",
    b"take",
    b"take_while",
    b"tally",
    b"tap",
    b"then",
    b"to_a",
    b"to_enum",
    b"to_h",
    b"to_s",
    b"to_set",
    b"uniq",
    b"values",
    b"values_at",
    b"yield_self",
    b"zip",
];

/// Checks unexpected overrides of the `Struct` built-in methods via
/// `Struct.new`.
///
/// # Examples
///
/// ```ruby
/// # bad
/// Bad = Struct.new(:members, :clone, :count)
/// b = Bad.new([], true, 1)
/// b.members #=> [] (overriding `Struct#members`)
/// b.clone #=> true (overriding `Object#clone`)
/// b.count #=> 1 (overriding `Enumerable#count`)
///
/// # good
/// Good = Struct.new(:id, :name)
/// g = Good.new(1, "foo")
/// g.members #=> [:id, :name]
/// g.clone #=> #<struct Good id=1, name="foo">
/// g.count #=> 2
/// ```
#[derive(Debug, Clone, Default)]
pub struct StructNewOverride;

/// RuboCop's `struct_new` node matcher: `Struct.new(...)`/`::Struct.new(...)`.
fn is_struct_new(call: &CallNode<'_>) -> bool {
    call.name().as_slice() == b"new"
        && call.receiver().is_some_and(|receiver| {
            is_bare_or_toplevel_const(&receiver)
                && const_name(&receiver).as_deref() == Some("Struct")
        })
}

/// Ruby's `Symbol#inspect`/`String#inspect` for one member-name argument:
/// `:name` for a `sym` node, `"name"` for a `str` node. Every fixture and
/// every real `STRUCT_METHOD_NAMES` entry is a plain identifier or operator
/// symbol, none of which `Symbol#inspect` ever quotes, so this never needs
/// upstream's quoting fallback for a symbol containing unusual characters.
fn inspect_member_name(arg: &Node<'_>, name: &[u8]) -> String {
    let text = String::from_utf8_lossy(name);
    if arg.kind() == NodeKind::SymbolNode {
        format!(":{text}")
    } else {
        format!("\"{text}\"")
    }
}

impl Rule for StructNewOverride {
    const META: RuleMeta = RuleMeta {
        name: "Lint/StructNewOverride",
        department: Department::Lint,
        summary: "Checks unexpected overrides of the `Struct` built-in methods via `Struct.new`.",
        explanation: "\
Checks unexpected overrides of the `Struct` built-in methods
via `Struct.new`.

```ruby
# bad
Bad = Struct.new(:members, :clone, :count)
b = Bad.new([], true, 1)
b.members #=> [] (overriding `Struct#members`)
b.clone #=> true (overriding `Object#clone`)
b.count #=> 1 (overriding `Enumerable#count`)

# good
Good = Struct.new(:id, :name)
g = Good.new(1, \"foo\")
g.members #=> [:id, :name]
g.clone #=> #<struct Good id=1, name=\"foo\">
g.count #=> 2
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
`STRUCT_METHOD_NAMES` is a hardcoded snapshot of `Struct.instance_methods` on Ruby 3.4.2 (see the
module doc) rather than a live computation; a member name that is a `Struct` instance method only
on some other Ruby version is not detected.",
    };

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Node::CallNode { .. } = node else { return };
        let call = node.as_call_node().expect("kind matched");
        if !is_struct_new(&call) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        for (index, arg) in arguments.arguments().iter().enumerate() {
            // Ignore if the first argument is a class name.
            if index == 0 && arg.kind() == NodeKind::StringNode {
                continue;
            }
            let name = match arg.kind() {
                NodeKind::SymbolNode => {
                    arg.as_symbol_node().expect("kind matched").unescaped().to_vec()
                }
                NodeKind::StringNode => {
                    arg.as_string_node().expect("kind matched").unescaped().to_vec()
                }
                _ => continue,
            };
            let name = name.as_slice();
            if !STRUCT_METHOD_NAMES.contains(&name) {
                continue;
            }
            let member_name = inspect_member_name(&arg, name);
            let method_name = String::from_utf8_lossy(name);
            let message = format!(
                "`{member_name}` member overrides `Struct#{method_name}` and it may be unexpected."
            );
            ctx.report(&Self::META, arg.span(), message);
        }
    }

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }
}
