//! `Gemspec/RubyVersionGlobalsUsage`, ported from RuboCop's
//! `lib/rubocop/cop/gemspec/ruby_version_globals_usage.rb` plus the
//! `GemspecHelp` mixin it includes.
//!
//! # Whole-file, not ancestor-based
//!
//! Upstream's `gem_spec_with_ruby_version?` is `gem_specification(processed_source.ast)
//! && ruby_version?(node)`. `gem_specification` is a `def_node_search`, which searches
//! the *entire* tree passed to it (here, the whole file's root) for any descendant
//! matching the `Gem::Specification.new do |x| ... end` pattern and returns truthy if
//! one exists anywhere -- it does not require the flagged `RUBY_VERSION`/`Ruby::VERSION`
//! node to be nested inside that block, only that such a block exists somewhere in the
//! same file. This port mirrors that literally: [`RubyVersionGlobalsUsage::file_start`]
//! scans the whole tree once for a matching block and every `RUBY_VERSION`/`Ruby::VERSION`
//! reference anywhere in the file is flagged if it found one, regardless of nesting.
//!
//! # Matching `RUBY_VERSION` / `Ruby::VERSION`
//!
//! Upstream's `ruby_version?` matcher is `{(const {cbase nil?} :RUBY_VERSION) (const (const
//! {cbase nil?} :Ruby) :VERSION)}`: a bare or top-level-qualified `RUBY_VERSION`, or a
//! `VERSION` constant path whose own namespace is a bare or top-level-qualified `Ruby` (and
//! no further namespace, e.g. `Foo::Ruby::VERSION` does not match). [`ruby_ast::ext::const_name`]
//! flattens a leading `::` (`cbase`) to nothing, so comparing its output against the literal
//! strings `"RUBY_VERSION"` and `"Ruby::VERSION"` reproduces exactly this shape: `RUBY_VERSION`,
//! `::RUBY_VERSION`, `Ruby::VERSION`, and `::Ruby::VERSION` all match, while `Foo::RUBY_VERSION`
//! and `Foo::Ruby::VERSION` do not (their flattened name carries the extra segment).
//!
//! # `Gem::Specification.new` block shape
//!
//! `GemspecHelp`'s pattern is `(block (send (const (const {cbase nil?} :Gem) :Specification)
//! :new) (args (arg $_)) ...)`: the block's owning call must be `Gem::Specification.new` (`Gem`'s
//! own namespace bare or top-level-qualified, matched here the same way as above via
//! `const_name` equalling `"Gem::Specification"`), and its parameter list must be exactly one
//! required positional parameter -- no optional/rest/post/keyword/block parameters -- captured
//! by [`is_single_required_param_block`] checking every `ParametersNode` list is empty except
//! `requireds`, which must have length one.
//!
//! Message: RuboCop's `%<ruby_version>s` is filled with `node.source`, i.e. the flagged node's
//! own exact source text (preserving a leading `::` or the `Ruby::` qualifier as written),
//! reproduced here via [`Context::text`].

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::const_name;
use ruby_ast::{each_descendant, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG_PREFIX: &str = "Do not use `";
const MSG_SUFFIX: &str = "` in gemspec file.";

/// Whether `node` (a `BlockNode`) is a `do |x| ... end`/`{ |x| ... }` block with exactly one
/// required positional parameter and no other parameter kinds -- RuboCop's `(args (arg $_))`.
fn is_single_required_param_block(node: &Node<'_>) -> bool {
    let Some(block) = node.as_block_node() else { return false };
    let Some(params) = block.parameters() else { return false };
    let Some(params) = params.as_block_parameters_node() else { return false };
    if !params.locals().is_empty() {
        return false;
    }
    let Some(inner) = params.parameters() else { return false };
    let requireds = inner.requireds();
    requireds.len() == 1
        && inner.optionals().is_empty()
        && inner.rest().is_none()
        && inner.posts().is_empty()
        && inner.keywords().is_empty()
        && inner.keyword_rest().is_none()
        && inner.block().is_none()
        && requireds.iter().next().is_some_and(|r| r.as_required_parameter_node().is_some())
}

/// RuboCop's `GemspecHelp#gem_specification` search predicate, applied to one `CallNode`:
/// `Gem::Specification.new` with an attached literal block taking exactly one required
/// parameter.
fn is_gem_specification_new(call_node: &Node<'_>) -> bool {
    let Some(call) = call_node.as_call_node() else { return false };
    if call.name().as_slice() != b"new" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    if const_name(&receiver).as_deref() != Some("Gem::Specification") {
        return false;
    }
    let Some(block) = call.block() else { return false };
    block.as_block_node().is_some() && is_single_required_param_block(&block)
}

/// Whether the flagged node's flattened constant name is `RUBY_VERSION` or `Ruby::VERSION`.
/// See the module doc's "Matching `RUBY_VERSION` / `Ruby::VERSION`" section.
fn is_ruby_version_reference(node: &Node<'_>) -> bool {
    matches!(const_name(node).as_deref(), Some("RUBY_VERSION" | "Ruby::VERSION"))
}

/// Checks that `RUBY_VERSION` and `Ruby::VERSION` constants are not used in gemspec.
///
/// Using `RUBY_VERSION` and `Ruby::VERSION` are dangerous because value of the constant is
/// determined by `rake release`. It's possible to have dependency based on ruby version used to
/// execute `rake release` and not user's ruby version.
///
/// # Examples
///
/// ```ruby
/// # bad
/// Gem::Specification.new do |spec|
///   if RUBY_VERSION >= '3.0'
///     spec.add_dependency 'gem_a'
///   else
///     spec.add_dependency 'gem_b'
///   end
/// end
///
/// # good
/// Gem::Specification.new do |spec|
///   spec.add_dependency 'gem_a'
/// end
/// ```
#[derive(Debug, Clone, Default)]
pub struct RubyVersionGlobalsUsage {
    /// Whether the file contains a `Gem::Specification.new do |x| ... end` block anywhere,
    /// computed once in [`Rule::file_start`] by scanning the whole tree (see the module doc).
    has_gem_specification: bool,
}

impl Rule for RubyVersionGlobalsUsage {
    const META: RuleMeta = RuleMeta {
        name: "Gemspec/RubyVersionGlobalsUsage",
        department: Department::Gemspec,
        summary: "Checks usage of RUBY_VERSION in gemspec.",
        explanation: "\
Checks that `RUBY_VERSION` and `Ruby::VERSION` constants are not used in gemspec.

Using `RUBY_VERSION` and `Ruby::VERSION` are dangerous because value of the constant is
determined by `rake release`. It's possible to have dependency based on ruby version used
to execute `rake release` and not user's ruby version.

```ruby
# bad
Gem::Specification.new do |spec|
  if RUBY_VERSION >= '3.0'
    spec.add_dependency 'gem_a'
  else
    spec.add_dependency 'gem_b'
  end
end

# good
Gem::Specification.new do |spec|
  spec.add_dependency 'gem_a'
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ConstantReadNode, NodeKind::ConstantPathNode],
        config: &[],
        blind_spots: "\
Upstream's `gem_specification` search does not require the flagged constant to be nested inside
the `Gem::Specification.new` block, only that such a block exists somewhere in the same file; this
port reproduces that literally (see the module doc), which is a real upstream quirk, not a
mistranslation. `Include: ['**/*.gemspec']` restricts this cop to gemspec files upstream; elysium
applies that restriction at the config-file-matching layer (see `crates/config`), not in this
rule's own logic.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.has_gem_specification = false;
        let root = ctx.parsed().root();
        let mut found = false;
        each_descendant(&root, &mut |node| {
            if !found && node.kind() == NodeKind::CallNode && is_gem_specification_new(node) {
                found = true;
            }
        });
        self.has_gem_specification = found;
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.has_gem_specification || !is_ruby_version_reference(node) {
            return;
        }
        let span = node.span();
        let source = String::from_utf8_lossy(ctx.text(span)).into_owned();
        ctx.report(&Self::META, span, format!("{MSG_PREFIX}{source}{MSG_SUFFIX}"));
    }
}
