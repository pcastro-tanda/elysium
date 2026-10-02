//! `Gemspec/DeprecatedAttributeAssignment`, ported from RuboCop's
//! `lib/rubocop/cop/gemspec/deprecated_attribute_assignment.rb`.
//!
//! Upstream matches `Gem::Specification.new do |x| ... end` locally (no
//! restriction on the block's parameter shape, unlike `GemspecHelp`'s own
//! matcher): `(block (send (const (const {cbase nil?} :Gem) :Specification)
//! :new) ...)`. [`gem_specification_call`] reproduces that shape, and
//! [`block_first_param_source`] extracts the raw source text of the
//! block's first parameter (whatever kind it is), matching upstream's
//! `block_node.first_argument.source`.
//!
//! `use_deprecated_attributes?` walks the block's descendants (depth-first,
//! pre-order -- the same order [`ruby_ast::each_descendant`] yields) looking
//! for the first one that is either a plain `receiver.attribute = value`
//! call (Prism: a `CallNode` whose name is one of `test_files=`, `date=`,
//! `specification_version=`, `rubygems_version=`) or a compound assignment
//! `receiver.attribute += value` (Prism: `CallOperatorWriteNode` whose
//! `read_name` is the bare attribute), with the receiver's own raw source
//! text equal (textually, not structurally -- upstream's `node.receiver&
//! .source == block_parameter`) to the block's first parameter's source
//! text. The whole matching node (the assignment or the compound-assignment
//! node, not just its receiver) is the offense range; autocorrect removes
//! it as a whole line, including the trailing newline.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// The four deprecated attributes, in upstream's checked order.
const DEPRECATED_ATTRIBUTES: &[&[u8]] =
    &[b"test_files", b"date", b"specification_version", b"rubygems_version"];

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

/// Whether `node` (a `.new` `CallNode`) is `Gem::Specification.new` with an attached block,
/// with no restriction on its parameter list's shape (upstream's local `gem_specification`
/// matcher, distinct from `GemspecHelp`'s own single-required-parameter one). Returns the
/// attached block node.
fn gem_specification_block<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"new" {
        return None;
    }
    let receiver = call.receiver()?;
    if !const_name_matches(&receiver, &[b"Gem", b"Specification"]) {
        return None;
    }
    let block = call.block()?;
    block.as_block_node()?;
    Some(block)
}

/// RuboCop's `block_node.first_argument.source`: the raw source span of the block's first
/// parameter of any kind (required/optional/rest/post/keyword/keyword-rest/block), or `None`
/// if the block takes no parameters.
fn block_first_param_span(node: &Node<'_>) -> Option<Span> {
    let block = node.as_block_node()?;
    let params = block.parameters()?;
    let params = params.as_block_parameters_node()?;
    let inner = params.parameters()?;
    inner
        .requireds()
        .iter()
        .next()
        .map(|n| n.span())
        .or_else(|| inner.optionals().iter().next().map(|n| n.span()))
        .or_else(|| inner.rest().map(|n| n.span()))
        .or_else(|| inner.posts().iter().next().map(|n| n.span()))
        .or_else(|| inner.keywords().iter().next().map(|n| n.span()))
        .or_else(|| inner.keyword_rest().map(|n| n.span()))
        .or_else(|| inner.block().map(|b| b.location().span()))
}

/// Checks that deprecated attributes are not set in a gemspec file.
#[derive(Debug, Clone)]
pub struct DeprecatedAttributeAssignment;

impl Rule for DeprecatedAttributeAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Gemspec/DeprecatedAttributeAssignment",
        department: Department::Gemspec,
        summary: "Checks that deprecated attributes are not set in a gemspec file.",
        explanation: "\
Checks that deprecated attributes are not set in a gemspec file.
Removing deprecated attributes allows the user to receive smaller packed
gems.

```ruby
# bad
Gem::Specification.new do |spec|
  spec.name = 'your_cool_gem_name'
  spec.test_files = Dir.glob('test/**/*')
end

# bad
Gem::Specification.new do |spec|
  spec.name = 'your_cool_gem_name'
  spec.test_files += Dir.glob('test/**/*')
end

# good
Gem::Specification.new do |spec|
  spec.name = 'your_cool_gem_name'
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(block) = gem_specification_block(node) else { return };
        let Some(param_span) = block_first_param_span(&block) else { return };
        let block_parameter = ctx.text(param_span).to_vec();

        let mut found: Option<(Span, &'static [u8])> = None;
        each_descendant(&block, &mut |descendant: &Node<'_>| {
            if found.is_some() {
                return;
            }
            match descendant.kind() {
                NodeKind::CallNode => {
                    let Some(call) = descendant.as_call_node() else { return };
                    let name = call.name().as_slice();
                    let Some(attribute) = DEPRECATED_ATTRIBUTES.iter().copied().find(|attr| {
                        name.len() == attr.len() + 1
                            && name.starts_with(attr)
                            && name.ends_with(b"=")
                    }) else {
                        return;
                    };
                    let Some(receiver) = call.receiver() else { return };
                    if ctx.text(receiver.span()) != block_parameter.as_slice() {
                        return;
                    }
                    found = Some((descendant.span(), attribute));
                }
                NodeKind::CallOperatorWriteNode => {
                    let Some(write) = descendant.as_call_operator_write_node() else { return };
                    let name = write.read_name().as_slice();
                    let Some(attribute) =
                        DEPRECATED_ATTRIBUTES.iter().copied().find(|attr| *attr == name)
                    else {
                        return;
                    };
                    let Some(receiver) = write.receiver() else { return };
                    if ctx.text(receiver.span()) != block_parameter.as_slice() {
                        return;
                    }
                    found = Some((descendant.span(), attribute));
                }
                _ => {}
            }
        });

        let Some((assignment_span, attribute)) = found else { return };
        let attribute = String::from_utf8_lossy(attribute);
        let message = format!("Do not set `{attribute}` in gemspec.");
        let range = ctx.whole_lines(assignment_span);
        let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(range)] };
        ctx.report_with_fix(&Self::META, assignment_span, message, fix);
    }
}
