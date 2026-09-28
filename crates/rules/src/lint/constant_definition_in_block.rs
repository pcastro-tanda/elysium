//! `Lint/ConstantDefinitionInBlock`, ported from RuboCop's
//! `lib/rubocop/cop/lint/constant_definition_in_block.rb` plus the
//! `AllowedMethods` mixin it includes.
//!
//! # Ancestor shape
//!
//! Upstream's two node-pattern matchers both boil down to "the node's parent
//! is any block, or its parent is a `begin` (implicit multi-statement body)
//! whose own parent is any block" -- two branches because whitequark's parser
//! only wraps a block body in a `begin` node when it holds more than one
//! statement, leaving a single-statement body's statement as the block's
//! direct child. Prism has no such special case: `BlockNode::body` is always
//! a `StatementsNode`, holding one statement or several alike (see
//! `UselessAccessModifier`'s `in_macro_scope` for the same observation). The
//! two upstream branches therefore collapse into one Prism check: the node's
//! parent is a `StatementsNode` and *that* node's parent is a `BlockNode`.
//! Unlike `in_macro_scope`, upstream's pattern here does not recurse through
//! intervening `if`/`unless`/`kwbegin` -- so `describe do; if x; FOO = 1; end;
//! end` is (deliberately, on both sides of the port) never flagged.
//!
//! `any_block_type?` covers a plain block, a numblock (`_1`), and an itblock
//! (bare `it`) alike; Prism represents all three as the same `BlockNode` kind
//! (they only differ in the shape of the node its `parameters` field points
//! at), so no extra handling is needed for the numbered/`it`-param fixtures.
//!
//! `casgn`'s `nil? ...` clause requires the constant have no explicit
//! namespace (`self::FOO`/`::FOO`/`A::FOO` are all exempt); Prism spells a
//! bare constant assignment as its own node kind, `ConstantWriteNode`, versus
//! a namespaced one's `ConstantPathWriteNode` -- so subscribing to only the
//! former reproduces that clause for free, with no explicit namespace check.
//! `module_defined_in_block?` (shared by `on_class`/`on_module`) has no such
//! clause.
//!
//! # `method_name`
//!
//! Upstream's `node.ancestors.find(&:any_block_type?).method_name` walks
//! *up* from the flagged node to the nearest enclosing block and asks its
//! owning `send` node for its method name. This port instead walks *down*:
//! [`ConstantDefinitionInBlock`] keeps a stack of owning-call method names,
//! pushed on entering a `CallNode` that has a literal block (skipping a
//! `&block` argument pass, which is a `BlockArgumentNode`, not a
//! `BlockNode`, and does not count as `any_block_type?` upstream either) and
//! popped on leaving it; the top of that stack, whenever a flagged node is
//! visited, is exactly the nearest enclosing block's owning method name.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, OptionError, Rule, RuleMeta, RuleOptions,
    Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Do not define constants this way within a block.";

/// Checks for the definition of constants within a block.
///
/// Do not define constants within a block, since the block's scope does not
/// isolate or namespace the constant in any way.
///
/// If you are trying to define that constant once, define it outside of
/// the block instead, or use a variable or method if defining the constant
/// in the outer scope would be problematic.
///
/// For meta-programming, use `const_set`.
///
/// # Examples
///
/// ```ruby
/// # bad
/// task :lint do
///   FILES_TO_LINT = Dir['lib/*.rb']
/// end
///
/// # bad
/// describe 'making a request' do
///   class TestRequest; end
/// end
///
/// # bad
/// module M
///   extend ActiveSupport::Concern
///   included do
///     LIST = []
///   end
/// end
///
/// # good
/// task :lint do
///   files_to_lint = Dir['lib/*.rb']
/// end
///
/// # good
/// describe 'making a request' do
///   let(:test_request) { Class.new }
///   # see also `stub_const` for RSpec
/// end
///
/// # good
/// module M
///   extend ActiveSupport::Concern
///   included do
///     const_set(:LIST, [])
///   end
/// end
/// ```
///
/// `AllowedMethods` (default: `enums`)
///
/// ```ruby
/// # good
///
/// # `enums` for Typed Enums via `T::Enum` in Sorbet.
/// # https://sorbet.org/docs/tenum
/// class TestEnum < T::Enum
///   enums do
///     Foo = new("foo")
///   end
/// end
/// ```
#[derive(Debug, Clone)]
pub struct ConstantDefinitionInBlock {
    /// `AllowedMethods`.
    allowed_methods: Vec<String>,
    /// Spans of the message (`message_loc`) of every `CallNode` with a
    /// literal block currently being walked into, innermost last. Text is
    /// read via `ctx.text` only when a constant/class/module is actually
    /// nested. See the module doc's "`method_name`" section.
    block_owner_stack: Vec<Span>,
}

impl ConstantDefinitionInBlock {
    /// RuboCop's `allowed_method?`, applied to the nearest enclosing block's
    /// owning method name.
    fn allowed_method(&self, ctx: &Context<'_>, span: Span) -> bool {
        let name = String::from_utf8_lossy(ctx.text(span));
        self.allowed_methods.iter().any(|m| m == name.as_ref())
    }

    /// RuboCop's `constant_assigned_in_block?`/`module_defined_in_block?`
    /// ancestor shape, collapsed for Prism. See the module doc.
    fn in_block_body(ctx: &Context<'_>) -> bool {
        let ancestors = ctx.ancestors();
        let len = ancestors.len();
        len >= 2
            && ancestors[len - 1].kind == NodeKind::StatementsNode
            && ancestors[len - 2].kind == NodeKind::BlockNode
    }

    /// Whether a `CallNode` owns a literal block (as opposed to a `&block`
    /// argument pass, which does not count as `any_block_type?` upstream).
    fn owns_literal_block(node: &Node<'_>) -> bool {
        node.as_call_node()
            .is_some_and(|call| call.block().is_some_and(|b| b.as_block_node().is_some()))
    }

    fn check(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !Self::in_block_body(ctx) {
            return;
        }
        if self.block_owner_stack.last().is_some_and(|&span| self.allowed_method(ctx, span)) {
            return;
        }
        ctx.report(&Self::META, node.span(), MSG);
    }
}

impl Rule for ConstantDefinitionInBlock {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ConstantDefinitionInBlock",
        department: Department::Lint,
        summary: "Checks for the definition of constants within a block.",
        explanation: "\
Do not define constants within a block, since the block's scope does not
isolate or namespace the constant in any way.

If you are trying to define that constant once, define it outside of the
block instead, or use a variable or method if defining the constant in the
outer scope would be problematic.

For meta-programming, use `const_set`.

```ruby
# bad
task :lint do
  FILES_TO_LINT = Dir['lib/*.rb']
end

# bad
describe 'making a request' do
  class TestRequest; end
end

# bad
module M
  extend ActiveSupport::Concern
  included do
    LIST = []
  end
end

# good
task :lint do
  files_to_lint = Dir['lib/*.rb']
end

# good
describe 'making a request' do
  let(:test_request) { Class.new }
  # see also `stub_const` for RSpec
end

# good
module M
  extend ActiveSupport::Concern
  included do
    const_set(:LIST, [])
  end
end
```

With `AllowedMethods: ['enums']` (the default):

```ruby
# good

# `enums` for Typed Enums via `T::Enum` in Sorbet.
# https://sorbet.org/docs/tenum
class TestEnum < T::Enum
  enums do
    Foo = new(\"foo\")
  end
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: linter::FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ClassNode,
            NodeKind::ModuleNode,
        ],
        config: &[ConfigOption {
            name: "AllowedMethods",
            default: ConfigDefault::StrList(&["enums"]),
            allowed: &[],
            doc: "Method names owning a block whose constants/classes/modules are never flagged.",
        }],
        blind_spots: "\
Upstream's ancestor pattern does not recurse through intervening `if`/
`unless`/`kwbegin` between a constant/class/module and its enclosing block,
so `describe do; if x; FOO = 1; end; end` is never flagged on either side of
this port. `AllowedMethods` is matched only against the exact enclosing
block's owning method name (no `IgnoredMethods`/`ExcludedMethods` deprecated
aliases, which upstream also merges in only via the shared, but here unused,
`AllowedMethods` mixin default).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allowed_methods: options.str_list("AllowedMethods"),
            block_owner_stack: Vec::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.block_owner_stack.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                if Self::owns_literal_block(node) {
                    let call = node.as_call_node().expect("kind matched");
                    let span = call.message_loc().map_or_else(|| node.span(), |loc| loc.span());
                    self.block_owner_stack.push(span);
                }
            }
            NodeKind::ConstantWriteNode | NodeKind::ClassNode | NodeKind::ModuleNode => {
                self.check(node, ctx);
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.kind() == NodeKind::CallNode && Self::owns_literal_block(node) {
            self.block_owner_stack.pop();
        }
    }
}
