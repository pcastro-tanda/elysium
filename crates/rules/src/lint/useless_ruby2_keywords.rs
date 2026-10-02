//! `Lint/UselessRuby2Keywords`, ported from RuboCop's
//! `lib/rubocop/cop/lint/useless_ruby2_keywords.rb`.
//!
//! # A dedicated root-to-leaf walk, not the shared `enter`/`leave` stream
//!
//! Upstream's `find_method_definition` walks `node.each_ancestor`, and at
//! each ancestor enumerates *that ancestor's own direct children*
//! (`each_child_node(:def, :any_block)`) looking for a matching
//! `def`/`define_method` sibling -- entirely independent of whether that
//! sibling appears before or after the `ruby2_keywords :foo` call in
//! source. [`Context::ancestors`] only exposes `NodeInfo` (kind/span, no
//! field access), which cannot enumerate a typed ancestor's children. This
//! rule therefore ignores the shared per-rule `enter`/`leave` stream
//! entirely (`kinds: &[]`) and instead drives its own traversal from
//! `file_end`, via [`ruby_ast::walk`] over [`Context::parsed`]'s root,
//! maintaining a private stack of real ancestor [`Node`]s (cheap: every
//! Prism node is `Copy`). [`for_each_child`] on each stacked ancestor then
//! reproduces `each_child_node` exactly, order-independent.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::{CallNode, DefNode, ParametersNode, SymbolNode};
use ruby_ast::{for_each_child, walk, LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

/// Finds unnecessary uses of `ruby2_keywords`.
#[derive(Debug, Clone)]
pub struct UselessRuby2Keywords;

impl Rule for UselessRuby2Keywords {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UselessRuby2Keywords",
        department: Department::Lint,
        summary: "Finds unnecessary uses of `ruby2_keywords`.",
        explanation: "\
Looks for `ruby2_keywords` calls for methods that do not need it.

`ruby2_keywords` should only be called on methods that accept an argument
splat (`*args`) but do not have explicit keyword arguments (`k:` or `k:
true`) or a keyword splat (`**kwargs`).

```ruby
# good (splat argument without keyword arguments)
ruby2_keywords def foo(*args); end

# bad (no arguments)
ruby2_keywords def foo; end

# bad (positional argument)
ruby2_keywords def foo(arg); end

# bad (double splatted argument)
ruby2_keywords def foo(**args); end

# bad (keyword arguments)
ruby2_keywords def foo(i:, j:); end

# bad (splat argument with keyword arguments)
ruby2_keywords def foo(*args, i:, j:); end

# bad (ruby2_keywords given a symbol)
def foo; end
ruby2_keywords :foo

# bad (ruby2_keywords with dynamic method)
define_method(:foo) { |arg| }
ruby2_keywords :foo
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let mut walker = Walker { ancestors: Vec::new(), offenses: Vec::new() };
        walk(&root, &mut walker);
        for (span, method_name) in walker.offenses {
            let message = format!("`ruby2_keywords` is unnecessary for method `{method_name}`.");
            ctx.report(&Self::META, span, message);
        }
    }
}

/// See the module doc comment.
struct Walker<'pr> {
    ancestors: Vec<Node<'pr>>,
    offenses: Vec<(Span, String)>,
}

impl<'pr> Visitor<'pr> for Walker<'pr> {
    fn enter(&mut self, node: &Node<'pr>) {
        if let Node::CallNode { .. } = node {
            let call = node.as_call_node().expect("kind matched");
            if call.receiver().is_none() && call.name().as_slice() == b"ruby2_keywords" {
                if let Some(offense) = resolve(&self.ancestors, &call) {
                    self.offenses.push(offense);
                }
            }
        }
        self.ancestors.push(*node);
    }

    fn leave(&mut self, _node: &Node<'pr>) {
        self.ancestors.pop();
    }
}

/// A matched `def`/`define_method` definition's own parameter shape.
enum Definition<'pr> {
    Def(Option<ParametersNode<'pr>>),
    Block(Option<Node<'pr>>),
}

/// RuboCop's `on_send`.
fn resolve<'pr>(ancestors: &[Node<'pr>], call: &CallNode<'pr>) -> Option<(Span, String)> {
    let first_argument = call.arguments().and_then(|args| args.arguments().first())?;
    if let Some(def_node) = first_argument.as_def_node() {
        return inspect_def(call, &def_node);
    }
    let sym = first_argument.as_symbol_node()?;
    inspect_sym(ancestors, call, &sym)
}

/// RuboCop's `inspect_def`.
fn inspect_def(call: &CallNode<'_>, def_node: &DefNode<'_>) -> Option<(Span, String)> {
    if allowed_def_params(def_node.parameters()) {
        return None;
    }
    let selector = call.message_loc().unwrap_or_else(|| call.location());
    let method_name = String::from_utf8_lossy(def_node.name().as_slice()).into_owned();
    Some((selector.span(), method_name))
}

/// RuboCop's `inspect_sym`.
fn inspect_sym<'pr>(
    ancestors: &[Node<'pr>],
    call: &CallNode<'pr>,
    sym: &SymbolNode<'pr>,
) -> Option<(Span, String)> {
    if ancestors.is_empty() {
        return None;
    }
    let method_name = sym.unescaped();
    let definition = find_method_definition(ancestors, method_name)?;
    if is_allowed(&definition) {
        return None;
    }
    let name = String::from_utf8_lossy(method_name).into_owned();
    Some((call.location().span(), name))
}

/// RuboCop's `find_method_definition`.
fn find_method_definition<'pr>(
    ancestors: &[Node<'pr>],
    method_name: &[u8],
) -> Option<Definition<'pr>> {
    for ancestor in ancestors.iter().rev() {
        let mut found = None;
        for_each_child(ancestor, |child| {
            if found.is_some() {
                return;
            }
            if let Some(def) = child.as_def_node() {
                if def.name().as_slice() == method_name {
                    found = Some(Definition::Def(def.parameters()));
                }
            } else if let Some(call) = child.as_call_node() {
                if is_define_method_call(&call, method_name) {
                    if let Some(block) = call.block().and_then(|b| b.as_block_node()) {
                        found = Some(Definition::Block(block.parameters()));
                    }
                }
            }
        });
        if found.is_some() {
            return found;
        }
        if matches!(
            ancestor.kind(),
            NodeKind::ClassNode | NodeKind::ModuleNode | NodeKind::SingletonClassNode
        ) {
            return None;
        }
    }
    None
}

/// RuboCop's `method_definition?`'s `any_block` alternative:
/// `(send _ :define_method (sym %1))`.
fn is_define_method_call(call: &CallNode<'_>, method_name: &[u8]) -> bool {
    if call.receiver().is_some() || call.name().as_slice() != b"define_method" {
        return false;
    }
    call.arguments()
        .and_then(|args| args.arguments().first())
        .and_then(|first| first.as_symbol_node())
        .is_some_and(|sym| sym.unescaped() == method_name)
}

fn is_allowed(definition: &Definition<'_>) -> bool {
    match definition {
        Definition::Def(params) => allowed_def_params(*params),
        Definition::Block(params) => allowed_block_params(*params),
    }
}

/// RuboCop's `allowed_arguments?`, for a `def`'s (or a block's inner)
/// `ParametersNode`: a restarg present and no keyword arguments/keyword
/// rest.
fn allowed_def_params(params: Option<ParametersNode<'_>>) -> bool {
    let Some(params) = params else { return false };
    params.rest().is_some() && params.keywords().is_empty() && params.keyword_rest().is_none()
}

/// `allowed_def_params`, for a block's own `parameters` field: `None` (no
/// params), a numbered/`it` implicit single parameter (never a restarg),
/// or `BlockParametersNode` wrapping the actual `ParametersNode`.
fn allowed_block_params(params: Option<Node<'_>>) -> bool {
    let Some(params) = params else { return false };
    params.as_block_parameters_node().is_some_and(|bp| allowed_def_params(bp.parameters()))
}
