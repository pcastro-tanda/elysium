//! `Gemspec/AttributeAssignment`, ported from RuboCop's
//! `lib/rubocop/cop/gemspec/attribute_assignment.rb` plus the `GemspecHelp`
//! mixin it includes.
//!
//! See `Gemspec/DuplicatedAssignment`'s port for the shared helpers copied
//! here verbatim (`const_name_matches`, `single_required_param_name`,
//! `gem_specification_var`, `matches_gem_lvar`, `is_assignment_method`,
//! `is_literal`): same whole-file, first-found-block-variable search, same
//! `(send (lvar gem_var) _ ...)` / `(send (send (lvar gem_var) _) :[]=
//! literal? _)` node shapes.
//!
//! Upstream's `on_new_investigation` collects two attribute sets: regular
//! assignments (`spec.foo = ...`, method name minus trailing `=`) and
//! indexed assignments (`spec.foo[i] = ...`, keyed by the inner call's
//! method name) -- both as whole-file searches, not ancestor-based. It then
//! reports an offense on *every* indexed-assignment node whose attribute
//! also appears among the regular assignments (the intersection), leaving
//! attributes that are consistently one style or the other alone.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use consistent style for Gemspec attributes assignment.";

/// Recursively compares a constant (path) node's segments, innermost-first, against `segments`.
fn const_name_matches(node: &Node<'_>, segments: &[&[u8]]) -> bool {
    match node.kind() {
        NodeKind::ConstantReadNode => {
            let Some(c) = node.as_constant_read_node() else { return false };
            matches!(segments, [only] if c.name().as_slice() == *only)
        }
        NodeKind::ConstantPathNode => {
            let Some(path) = node.as_constant_path_node() else { return false };
            let Some((last, init)) = segments.split_last() else { return false };
            let Some(short) = path.name() else { return false };
            if short.as_slice() != *last {
                return false;
            }
            match path.parent() {
                Some(parent) => const_name_matches(&parent, init),
                None => init.is_empty(),
            }
        }
        _ => false,
    }
}

/// Whether `node` (a `BlockNode`) is a `do |x| ... end`/`{ |x| ... }` block with exactly one
/// required positional parameter and no other parameter kinds, returning the parameter's name
/// when it matches.
fn single_required_param_name(node: &Node<'_>) -> Option<Vec<u8>> {
    let block = node.as_block_node()?;
    let params = block.parameters()?;
    let params = params.as_block_parameters_node()?;
    if !params.locals().is_empty() {
        return None;
    }
    let inner = params.parameters()?;
    let requireds = inner.requireds();
    if requireds.len() != 1
        || !inner.optionals().is_empty()
        || inner.rest().is_some()
        || !inner.posts().is_empty()
        || !inner.keywords().is_empty()
        || inner.keyword_rest().is_some()
        || inner.block().is_some()
    {
        return None;
    }
    let req = requireds.iter().next()?.as_required_parameter_node()?;
    Some(req.name().as_slice().to_vec())
}

/// RuboCop's `GemspecHelp#gem_specification` search predicate, applied to one `CallNode`:
/// `Gem::Specification.new` with an attached literal block taking exactly one required
/// parameter, returning that parameter's name.
fn gem_specification_var(node: &Node<'_>) -> Option<Vec<u8>> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"new" {
        return None;
    }
    let receiver = call.receiver()?;
    if !const_name_matches(&receiver, &[b"Gem", b"Specification"]) {
        return None;
    }
    let block = call.block()?;
    single_required_param_name(&block)
}

/// Whether `node` is a `(lvar name)` read whose name is `_1`, `it`, or the captured
/// `Gem::Specification.new` block parameter name.
fn matches_gem_lvar(node: &Node<'_>, gem_var: Option<&[u8]>) -> bool {
    if node.as_it_local_variable_read_node().is_some() {
        return true;
    }
    let Some(lvar) = node.as_local_variable_read_node() else { return false };
    let name = lvar.name().as_slice();
    name == b"_1" || gem_var.is_some_and(|g| g == name)
}

/// RuboCop-AST's `assignment_method?`: a method name ending in `=` that is not one of the
/// comparison operators (`comparison_method?`).
fn is_assignment_method(name: &[u8]) -> bool {
    name.ends_with(b"=") && !matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=")
}

/// RuboCop-AST's `Node::LITERALS`: string/symbol/int/float/array/hash/regexp/true/false/nil/
/// range/rational/complex node kinds, including their interpolated/dynamic forms.
fn is_literal(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::RationalNode
            | NodeKind::ImaginaryNode
            | NodeKind::ArrayNode
            | NodeKind::HashNode
            | NodeKind::RegularExpressionNode
            | NodeKind::InterpolatedRegularExpressionNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::RangeNode
    )
}

/// One `receiver.method[literal] = value`-shaped indexed-assignment candidate.
#[derive(Debug, Clone)]
struct IndexedEntry {
    attribute: Vec<u8>,
    span: Span,
}

/// Use consistent style for Gemspec attributes assignment.
#[derive(Debug, Clone, Default)]
pub struct AttributeAssignment {
    gem_var: Option<Vec<u8>>,
    /// Attributes (method name minus trailing `=`) assigned via `spec.foo = ...`.
    regular_attrs: Vec<Vec<u8>>,
    indexed: Vec<IndexedEntry>,
}

impl Rule for AttributeAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Gemspec/AttributeAssignment",
        department: Department::Gemspec,
        summary: "Use consistent style for Gemspec attributes assignment.",
        explanation: "\
Use consistent style for Gemspec attributes assignment.

```ruby
# bad
# This example uses two styles for assignment of metadata attribute.
Gem::Specification.new do |spec|
  spec.metadata = { 'key' => 'value' }
  spec.metadata['another-key'] = 'another-value'
end

# good
Gem::Specification.new do |spec|
  spec.metadata['key'] = 'value'
  spec.metadata['another-key'] = 'another-value'
end

# good
Gem::Specification.new do |spec|
  spec.metadata = { 'key' => 'value', 'another-key' => 'another-value' }
end
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Upstream's `match_block_variable_name?` `return`s from inside a `def_node_search` block, so
only the *first* `Gem::Specification.new do |x| ... end` block found anywhere in the file is
ever consulted for its parameter name; a literal upstream quirk reproduced here rather than
fixed. `Include: ['**/*.gemspec']` restricts this cop to gemspec files upstream; elysium applies
that restriction at the config-file-matching layer, not in this rule's own logic.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let _ = ctx;
        self.gem_var = None;
        self.regular_attrs.clear();
        self.indexed.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let _ = ctx;
        let Some(call) = node.as_call_node() else { return };

        if self.gem_var.is_none() {
            if let Some(var) = gem_specification_var(node) {
                self.gem_var = Some(var);
            }
        }
        let gem_var = self.gem_var.clone();
        let gem_var = gem_var.as_deref();

        if call.is_safe_navigation() {
            return;
        }

        if let Some(receiver) = call.receiver() {
            if matches_gem_lvar(&receiver, gem_var) {
                let method = call.name().as_slice().to_vec();
                if is_assignment_method(&method) {
                    let attribute = method[..method.len() - 1].to_vec();
                    self.regular_attrs.push(attribute);
                }
            }

            if call.name().as_slice() == b"[]=" {
                if let Some(inner) = receiver.as_call_node() {
                    if !inner.is_safe_navigation() && inner.arguments().is_none() {
                        if let Some(inner_receiver) = inner.receiver() {
                            if matches_gem_lvar(&inner_receiver, gem_var) {
                                if let Some(args) = call.arguments() {
                                    let items: Vec<Node<'_>> = args.arguments().iter().collect();
                                    if items.len() == 2 && is_literal(items[0].kind()) {
                                        let attribute = inner.name().as_slice().to_vec();
                                        self.indexed
                                            .push(IndexedEntry { attribute, span: node.span() });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        if ctx.source().bytes().iter().all(u8::is_ascii_whitespace) {
            return;
        }
        for entry in &self.indexed {
            if self.regular_attrs.iter().any(|a| a == &entry.attribute) {
                ctx.report(&Self::META, entry.span, MSG);
            }
        }
    }
}
