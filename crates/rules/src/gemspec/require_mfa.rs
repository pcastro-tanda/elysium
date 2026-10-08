//! `Gemspec/RequireMFA`, ported from RuboCop's
//! `lib/rubocop/cop/gemspec/require_mfa.rb` plus the `GemspecHelp` mixin's
//! strict (single-required-parameter) `gem_specification?` matcher it uses.
//!
//! Each `on_block` invocation processes one `Gem::Specification.new do |x|
//! ... end` block in isolation (unlike `Gemspec/DuplicatedAssignment`'s
//! whole-file-first-found-variable quirk): [`metadata_value`] searches the
//! block's whole subtree (any receiver, not just the block's own
//! parameter -- upstream's `metadata`/`metadata_assignment` matchers both
//! use a bare `_` receiver) for the first of either `_.metadata = VALUE` or
//! `_.metadata[('rubygems_mfa_required'|:rubygems_mfa_required)] = VALUE`,
//! capturing `VALUE`. [`mfa_value`] then either takes that value directly
//! (when it is itself a plain string, i.e. came from the indexed-write
//! branch) or searches *within* it (deep, including nested hashes) for a
//! `'rubygems_mfa_required' => VALUE`/`rubygems_mfa_required: VALUE` pair,
//! via [`find_pair_value`].
//!
//! If a value was found and is not the literal string `'true'`, the
//! offense and (safe) autocorrect replace just that value node. Otherwise
//! (no value found anywhere) the offense covers the whole block, and
//! autocorrect either inserts a `'rubygems_mfa_required' => 'true'` pair
//! into an existing metadata hash literal ([`correct_metadata`]: after the
//! last pair, or before the closing brace if there are none), skips
//! correction entirely when a `metadata = non_hash_value` assignment
//! exists (upstream's `return unless metadata.hash_type?`), or inserts a
//! whole new `spec.metadata['rubygems_mfa_required'] = 'true'` statement
//! after the block's own last `metadata` assignment of either shape
//! ([`collect_metadata_assignments`]), or just before the block's closing
//! `end` if there is no metadata assignment at all.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "`metadata['rubygems_mfa_required']` must be set to `'true'`.";

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

/// `GemspecHelp#gem_specification?`: a `do |x| ... end`/`{ |x| ... }` block with exactly one
/// required positional parameter and no other parameter kinds, returning the parameter's own
/// span (for its source text) when it matches.
fn single_required_param_span(node: &Node<'_>) -> Option<Span> {
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
    Some(requireds.iter().next()?.span())
}

/// `Gem::Specification.new` with an attached block taking exactly one required parameter,
/// returning that block node.
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
    single_required_param_span(&block)?;
    Some(block)
}

/// Whether `node` is the string `"rubygems_mfa_required"` or the symbol `:rubygems_mfa_required`
/// -- upstream's `{(str "rubygems_mfa_required") (sym :rubygems_mfa_required)}`.
fn is_mfa_key(node: &Node<'_>) -> bool {
    if let Some(s) = node.as_string_node() {
        return s.unescaped() == b"rubygems_mfa_required";
    }
    if let Some(s) = node.as_symbol_node() {
        return s.unescaped() == b"rubygems_mfa_required";
    }
    false
}

/// Whether `node` is any plain string or symbol literal -- upstream's bare `{str sym}`.
fn is_str_or_sym(node: &Node<'_>) -> bool {
    node.as_string_node().is_some() || node.as_symbol_node().is_some()
}

/// Whether `node` (a `CallNode`) is `_.metadata[KEY] = VALUE` for some literal `KEY`, where
/// `key_ok` decides whether `KEY` qualifies; returns `(key_node, value_node)`.
fn indexed_metadata_write<'a>(
    node: &Node<'a>,
    key_ok: impl Fn(&Node<'a>) -> bool,
) -> Option<(Node<'a>, Node<'a>)> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"[]=" {
        return None;
    }
    let receiver = call.receiver()?;
    let inner = receiver.as_call_node()?;
    if inner.name().as_slice() != b"metadata" || inner.arguments().is_some() {
        return None;
    }
    let args = call.arguments()?;
    let items: Vec<Node<'a>> = args.arguments().iter().collect();
    if items.len() != 2 || !key_ok(&items[0]) {
        return None;
    }
    Some((items[0], items[1]))
}

/// Upstream's `metadata` matcher: searches `root`'s whole subtree (any receiver) for the first
/// `_.metadata = VALUE` or `_.metadata[('rubygems_mfa_required'|:rubygems_mfa_required)] =
/// VALUE`, capturing `VALUE`.
fn metadata_value<'a>(root: &Node<'a>) -> Option<Node<'a>> {
    let mut found: Option<Node<'a>> = None;
    each_descendant(root, &mut |n: &Node<'a>| {
        if found.is_some() {
            return;
        }
        if let Some(call) = n.as_call_node() {
            if call.name().as_slice() == b"metadata=" {
                if let Some(args) = call.arguments() {
                    let items: Vec<Node<'a>> = args.arguments().iter().collect();
                    if items.len() == 1 {
                        found = Some(items[0]);
                    }
                }
                return;
            }
        }
        if let Some((_, value)) = indexed_metadata_write(n, is_mfa_key) {
            found = Some(value);
        }
    });
    found
}

/// Upstream's `metadata_assignment` search: every `_.metadata = VALUE` or `_.metadata[str/sym] =
/// VALUE` node (not just the MFA key) in `root`'s subtree, source order.
fn collect_metadata_assignments<'a>(root: &Node<'a>) -> Vec<Node<'a>> {
    let mut list = Vec::new();
    each_descendant(root, &mut |n: &Node<'a>| {
        if let Some(call) = n.as_call_node() {
            if call.name().as_slice() == b"metadata=" {
                list.push(*n);
                return;
            }
        }
        if indexed_metadata_write(n, is_str_or_sym).is_some() {
            list.push(*n);
        }
    });
    list
}

/// Upstream's `rubygems_mfa_required` search: the first `'rubygems_mfa_required' => VALUE` /
/// `rubygems_mfa_required: VALUE` pair anywhere within `root`'s subtree (deep, including nested
/// hashes), capturing `VALUE`.
fn find_pair_value<'a>(root: &Node<'a>) -> Option<Node<'a>> {
    let mut found: Option<Node<'a>> = None;
    each_descendant(root, &mut |n: &Node<'a>| {
        if found.is_some() {
            return;
        }
        let Some(assoc) = n.as_assoc_node() else { return };
        if is_mfa_key(&assoc.key()) {
            found = Some(assoc.value());
        }
    });
    found
}

/// Upstream's `mfa_value`.
fn mfa_value(metadata: Option<Node<'_>>) -> Option<Node<'_>> {
    let value = metadata?;
    if value.as_string_node().is_some() {
        return Some(value);
    }
    find_pair_value(&value)
}

/// Upstream's `true_string?`.
fn is_true_string(node: &Node<'_>) -> bool {
    node.as_string_node().is_some_and(|s| s.unescaped() == b"true")
}

/// Only `AssocNode` (`pair`) elements of a hash literal count -- `**splat` is excluded, matching
/// `HashNode#pairs`.
fn last_pair<'a>(hash: &ruby_ast::node::HashNode<'a>) -> Option<Node<'a>> {
    hash.elements().iter().filter(|el| el.as_assoc_node().is_some()).last()
}

/// Requires a gemspec to have `rubygems_mfa_required` metadata set.
#[derive(Debug, Clone)]
pub struct RequireMFA;

impl Rule for RequireMFA {
    const META: RuleMeta = RuleMeta {
        name: "Gemspec/RequireMFA",
        department: Department::Gemspec,
        summary: "Requires a gemspec to have `rubygems_mfa_required` metadata set.",
        explanation: "\
Requires a gemspec to have `rubygems_mfa_required` metadata set.

This setting tells RubyGems that MFA (Multi-Factor Authentication) is
required for accounts to be able to perform privileged operations, such
as `gem push`, `gem yank`, and adding or removing owners.

This helps make your gem more secure, as users can be more confident
that gem updates were pushed by maintainers.

```ruby
# bad
Gem::Specification.new do |spec|
  # no `rubygems_mfa_required` metadata specified
end

# good
Gem::Specification.new do |spec|
  spec.metadata = {
    'rubygems_mfa_required' => 'true'
  }
end

# good
Gem::Specification.new do |spec|
  spec.metadata['rubygems_mfa_required'] = 'true'
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
        let Some(param_span) = single_required_param_span(&block) else { return };
        let block_var = ctx.text(param_span).to_vec();

        let metadata = metadata_value(&block);
        let value = mfa_value(metadata);

        if let Some(value) = value {
            if is_true_string(&value) {
                return;
            }
            let fix = Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(value.span(), b"'true'".to_vec())],
            };
            ctx.report_with_fix(&Self::META, value.span(), MSG, fix);
            return;
        }

        let edits = autocorrect_edits(&block, &block_var, metadata.as_ref());
        match edits {
            Some(edits) => {
                let fix = Fix { applicability: Applicability::Safe, edits };
                ctx.report_with_fix(&Self::META, node.span(), MSG, fix);
            }
            None => {
                ctx.report(&Self::META, node.span(), MSG);
            }
        }
    }
}

/// Upstream's `autocorrect`/`correct_metadata`/`insert_mfa_required`. Returns `None` when
/// upstream's corrector block would run but perform no edit at all (the `return unless
/// metadata.hash_type?` early exit when a non-hash `metadata = ...` assignment already exists).
fn autocorrect_edits(
    block: &Node<'_>,
    block_var: &[u8],
    metadata: Option<&Node<'_>>,
) -> Option<Vec<Edit>> {
    if let Some(metadata) = metadata {
        let hash = metadata.as_hash_node()?;
        return Some(match last_pair(&hash) {
            Some(pair) => {
                vec![Edit::insert(
                    pair.span().end,
                    b",\n'rubygems_mfa_required' => 'true'".to_vec(),
                )]
            }
            None => {
                vec![Edit::insert(
                    hash.closing_loc().span().start,
                    b"'rubygems_mfa_required' => 'true'".to_vec(),
                )]
            }
        });
    }

    let block_var = String::from_utf8_lossy(block_var);
    let directive = format!("{block_var}.metadata['rubygems_mfa_required'] = 'true'");
    let assignments = collect_metadata_assignments(block);
    if let Some(last) = assignments.last() {
        return Some(vec![Edit::insert(last.span().end, format!("\n{directive}").into_bytes())]);
    }
    let block_node = block.as_block_node()?;
    Some(vec![Edit::insert(
        block_node.closing_loc().span().start,
        format!("{directive}\n").into_bytes(),
    )])
}
