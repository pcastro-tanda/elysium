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
//! same file. Rather than running a second, dedicated whole-tree walk (`each_descendant`)
//! per file to search for the block, this port reuses the single walk the engine already
//! performs for every subscribed node kind: [`RubyVersionGlobalsUsage::enter`] is also
//! subscribed to `CallNode` and flips [`RubyVersionGlobalsUsage::has_gem_specification`]
//! the first time it sees a matching block anywhere in the file, while every
//! `RUBY_VERSION`/`Ruby::VERSION` reference is collected into
//! [`RubyVersionGlobalsUsage::ruby_version_candidates`] regardless of nesting or of
//! whether the block has been seen yet. [`Rule::file_end`] then reports every collected
//! candidate iff the flag ended up set, reproducing the same file-global, order-independent
//! semantics as the two-pass version without a second tree traversal.
//!
//! # Matching `RUBY_VERSION` / `Ruby::VERSION`
//!
//! Upstream's `ruby_version?` matcher is `{(const {cbase nil?} :RUBY_VERSION) (const (const
//! {cbase nil?} :Ruby) :VERSION)}`: a bare or top-level-qualified `RUBY_VERSION`, or a
//! `VERSION` constant path whose own namespace is a bare or top-level-qualified `Ruby` (and
//! no further namespace, e.g. `Foo::Ruby::VERSION` does not match). [`const_name_matches`]
//! walks a constant (path) node's segments innermost-first against an expected segment list,
//! treating a leading `::` (`cbase`, no `parent`) as contributing no extra segment -- the same
//! shape [`ruby_ast::ext::const_name`] flattens to a string, but compared directly against
//! byte slices with no `String`/`Vec` allocation per node, since this runs on every constant
//! (path) node in every file rather than only on a rare match.
//!
//! # `Gem::Specification.new` block shape
//!
//! `GemspecHelp`'s pattern is `(block (send (const (const {cbase nil?} :Gem) :Specification)
//! :new) (args (arg $_)) ...)`: the block's owning call must be `Gem::Specification.new` (`Gem`'s
//! own namespace bare or top-level-qualified, matched here the same way as above via
//! [`const_name_matches`] against `["Gem", "Specification"]`), and its parameter list must be
//! exactly one required positional parameter -- no optional/rest/post/keyword/block parameters
//! -- captured by [`is_single_required_param_block`] checking every `ParametersNode` list is
//! empty except `requireds`, which must have length one.
//!
//! Message: RuboCop's `%<ruby_version>s` is filled with `node.source`, i.e. the flagged node's
//! own exact source text (preserving a leading `::` or the `Ruby::` qualifier as written),
//! reproduced here via [`Context::text`].

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG_PREFIX: &str = "Do not use `";
const MSG_SUFFIX: &str = "` in gemspec file.";

/// Recursively compares a constant (path) node's segments, innermost-first, against `segments`
/// with no `String`/`Vec` allocation -- see the module doc's "Matching `RUBY_VERSION` /
/// `Ruby::VERSION`" section. `segments` lists the expected path outer-to-inner (e.g.
/// `["Gem", "Specification"]` for `Gem::Specification`); a leading `::` (`cbase`, no `parent`)
/// contributes nothing extra, matching upstream's `cbase_type?` special case.
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
    if !const_name_matches(&receiver, &[b"Gem", b"Specification"]) {
        return false;
    }
    let Some(block) = call.block() else { return false };
    block.as_block_node().is_some() && is_single_required_param_block(&block)
}

/// Whether the flagged node's flattened constant name is `RUBY_VERSION` or `Ruby::VERSION`.
/// See the module doc's "Matching `RUBY_VERSION` / `Ruby::VERSION`" section.
fn is_ruby_version_reference(node: &Node<'_>) -> bool {
    const_name_matches(node, &[b"RUBY_VERSION"]) || const_name_matches(node, &[b"Ruby", b"VERSION"])
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
    /// set the first time [`Rule::enter`] sees one (see the module doc).
    has_gem_specification: bool,
    /// Every `RUBY_VERSION`/`Ruby::VERSION` reference span seen so far this file, reported in
    /// [`Rule::file_end`] iff `has_gem_specification` ended up set (see the module doc).
    ruby_version_candidates: Vec<Span>,
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
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::ConstantReadNode, NodeKind::ConstantPathNode],
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
        let _ = ctx;
        self.has_gem_specification = false;
        self.ruby_version_candidates.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let _ = ctx;
        match node.kind() {
            NodeKind::CallNode => {
                if !self.has_gem_specification && is_gem_specification_new(node) {
                    self.has_gem_specification = true;
                }
            }
            NodeKind::ConstantReadNode | NodeKind::ConstantPathNode
                if is_ruby_version_reference(node) =>
            {
                self.ruby_version_candidates.push(node.span());
            }
            _ => {}
        }
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        if !self.has_gem_specification {
            return;
        }
        for span in std::mem::take(&mut self.ruby_version_candidates) {
            let source = String::from_utf8_lossy(ctx.text(span)).into_owned();
            ctx.report(&Self::META, span, format!("{MSG_PREFIX}{source}{MSG_SUFFIX}"));
        }
    }
}
