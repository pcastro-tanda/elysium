//! `Gemspec/DuplicatedAssignment`, ported from RuboCop's
//! `lib/rubocop/cop/gemspec/duplicated_assignment.rb` plus the `GemspecHelp`
//! mixin it includes.
//!
//! # Whole-file search, not ancestor-based
//!
//! Upstream's `assignment_method_declarations`/`indexed_assignment_method_declarations`
//! are `def_node_search`s over `processed_source.ast` (the whole file), matching any
//! `(send (lvar name) ...)` where `name` is `_1`, `it`, or -- via the
//! `match_block_variable_name?` predicate -- the parameter name of the *first*
//! `Gem::Specification.new do |x| ... end` block found anywhere in the file (the predicate
//! `return`s from inside a search block, so only the first such block is ever consulted, a
//! literal upstream quirk this port reproduces). Rather than a dedicated whole-tree walk, this
//! reuses the single walk the engine already performs for every `CallNode`: [`enter`] records
//! the first matching block's parameter name into `gem_var` (set once, first-found, exactly
//! like upstream's early-return search) and collects every candidate assignment/indexed-write
//! call regardless of where `gem_var` currently stands, since -- as in the block-shape
//! reasoning below -- the opening `Gem::Specification.new` call is always visited before any
//! node inside its own block body in a pre-order walk. Grouping and reporting happen once, in
//! [`Rule::file_end`], mirroring upstream's `group_by(&:method_name).values.select { |nodes|
//! nodes.size > 1 }` two-pass shape.
//!
//! # `Gem::Specification.new` block shape
//!
//! Same as `Gemspec/RubyVersionGlobalsUsage`'s port: `(block (send (const (const {cbase nil?}
//! :Gem) :Specification) :new) (args (arg $_)) ...)` -- the block's owning call must be
//! `Gem::Specification.new` (`Gem`'s own namespace bare or top-level-qualified), and its
//! parameter list must be exactly one required positional parameter, captured (not merely
//! checked) here since its name is what later lvar reads must match.
//!
//! # Regular vs. indexed assignment
//!
//! `process_assignment_method_nodes` filters `assignment_method_declarations` (any
//! `receiver.method(...)` call on a matching lvar) down to `assignment_method?` calls -- a
//! method name ending in `=` that is not one of the comparison operators
//! (`== === != <= >=`) -- then groups by method name (Prism: [`CallNode::name`] already
//! includes the trailing `=` for attribute-write calls, e.g. `name=`).
//!
//! `process_indexed_assignment_method_nodes` matches `(send (send (lvar x) _) :[]= literal?
//! _)`: an outer `[]=` call whose receiver is itself a no-argument call on a matching lvar
//! (`spec.metadata`), with exactly two arguments, the first of which is a literal (RuboCop-AST's
//! `LITERALS`: string/symbol/int/float/array/hash/regexp/true/false/nil/range/rational/complex,
//! reproduced here by [`is_literal`]). Grouping uses `[node.children.first.method_name,
//! node.first_argument]` -- the *value* of the first argument via whitequark's structural node
//! equality, not its raw source text (so `'key'` twice groups, but `'key'` and `:key` do not) --
//! reproduced by [`literal_signature`], which discriminates by node kind plus (for strings and
//! symbols) unescaped bytes, falling back to raw source text for every other literal kind.
//!
//! # Messages and offense range
//!
//! `register_offense`'s range is `node.loc.column...node.loc.last_column` re-anchored to
//! `node.first_line` -- for the single-line calls this cop always deals with, that is simply
//! the whole call node's own span, reproduced here as `node.span()`. The message interpolates
//! either the bare method name (`name=`) or `method[key.source]=` (using the *reporting* node's
//! own key text, not the first occurrence's) and the first occurrence's `first_line`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`, split around the two `%<...>s`/`%<...>d` interpolations.
const MSG_PREFIX: &str = "`";
const MSG_MIDDLE: &str = "` method calls already given on line ";
const MSG_SUFFIX: &str = " of the gemspec.";

/// Recursively compares a constant (path) node's segments, innermost-first, against `segments`
/// -- see `Gemspec/RubyVersionGlobalsUsage`'s port for the same helper.
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
/// required positional parameter and no other parameter kinds -- RuboCop's `(args (arg $_))` --
/// returning the parameter's name when it matches.
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
/// `Gem::Specification.new` block parameter name -- RuboCop's `{#match_block_variable_name?
/// :_1 :it}`.
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

/// A grouping key for a literal node's *value* (not its raw source text), so `'key'` groups
/// with another `'key'` but not with `:key` or `"key"` written differently -- see the module
/// doc's "Regular vs. indexed assignment" section. `None` when `node` is not a literal at all.
fn literal_signature(node: &Node<'_>, ctx: &Context<'_>) -> Option<Vec<u8>> {
    let kind = node.kind();
    if !is_literal(kind) {
        return None;
    }
    let mut sig = format!("{kind:?}:").into_bytes();
    match kind {
        NodeKind::StringNode => sig.extend_from_slice(node.as_string_node()?.unescaped()),
        NodeKind::SymbolNode => sig.extend_from_slice(node.as_symbol_node()?.unescaped()),
        NodeKind::TrueNode | NodeKind::FalseNode | NodeKind::NilNode => {}
        _ => sig.extend_from_slice(ctx.text(node.span())),
    }
    Some(sig)
}

/// One `receiver.method = value`-shaped duplicate candidate.
#[derive(Debug, Clone)]
struct AssignmentEntry {
    method: Vec<u8>,
    span: Span,
    line: u32,
}

/// One `receiver.method[literal] = value`-shaped duplicate candidate.
#[derive(Debug, Clone)]
struct IndexedEntry {
    method: Vec<u8>,
    key_sig: Vec<u8>,
    key_source: Vec<u8>,
    span: Span,
    line: u32,
}

/// An attribute assignment method calls should be listed only once in a gemspec.
#[derive(Debug, Clone, Default)]
pub struct DuplicatedAssignment {
    /// The parameter name of the first `Gem::Specification.new do |x| ... end` block found
    /// anywhere in the file, set once (see the module doc's "Whole-file search" section).
    gem_var: Option<Vec<u8>>,
    assignments: Vec<AssignmentEntry>,
    indexed: Vec<IndexedEntry>,
}

impl Rule for DuplicatedAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Gemspec/DuplicatedAssignment",
        department: Department::Gemspec,
        summary: "An attribute assignment method calls should be listed only once in a gemspec.",
        explanation: "\
Assigning to an attribute with the same name using `spec.foo =` or
`spec.attribute#[]=` will be an unintended usage. On the other hand,
duplication of methods such as `spec.requirements`,
`spec.add_runtime_dependency`, and others are permitted because it is
the intended use of appending values.

```ruby
# bad
Gem::Specification.new do |spec|
  spec.name = 'rubocop'
  spec.name = 'rubocop2'
end

# good
Gem::Specification.new do |spec|
  spec.name = 'rubocop'
end

# good
Gem::Specification.new do |spec|
  spec.requirements << 'libmagick, v6.0'
  spec.requirements << 'A good graphics card'
end

# good
Gem::Specification.new do |spec|
  spec.add_dependency('parallel', '~> 1.10')
  spec.add_dependency('parser', '>= 2.3.3.1', '< 3.0')
end

# bad
Gem::Specification.new do |spec|
  spec.metadata[\"key\"] = \"value\"
  spec.metadata[\"key\"] = \"value\"
end

# good
Gem::Specification.new do |spec|
  spec.metadata[\"key\"] = \"value\"
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Upstream's `match_block_variable_name?` `return`s from inside a `def_node_search` block, so
only the *first* `Gem::Specification.new do |x| ... end` block found anywhere in the file is
ever consulted for its parameter name; a file with two such blocks using different parameter
names only recognizes the first one's, a literal upstream quirk reproduced here rather than
fixed. `Include: ['**/*.gemspec']` restricts this cop to gemspec files upstream; elysium applies
that restriction at the config-file-matching layer (see `crates/config`), not in this rule's own
logic.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let _ = ctx;
        self.gem_var = None;
        self.assignments.clear();
        self.indexed.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };

        if self.gem_var.is_none() {
            if let Some(var) = gem_specification_var(node) {
                self.gem_var = Some(var);
            }
        }

        let Some(receiver) = call.receiver() else { return };
        let gem_var = self.gem_var.as_deref();

        if matches_gem_lvar(&receiver, gem_var) {
            let method = call.name().as_slice().to_vec();
            if is_assignment_method(&method) {
                let line = ctx.line_col(node.span().start).line;
                self.assignments.push(AssignmentEntry { method, span: node.span(), line });
            }
        }

        if call.name().as_slice() == b"[]=" && !call.is_safe_navigation() {
            if let Some(inner) = receiver.as_call_node() {
                if !inner.is_safe_navigation() && inner.arguments().is_none() {
                    if let Some(inner_receiver) = inner.receiver() {
                        if matches_gem_lvar(&inner_receiver, gem_var) {
                            if let Some(args) = call.arguments() {
                                let items: Vec<Node<'_>> = args.arguments().iter().collect();
                                if items.len() == 2 {
                                    if let Some(sig) = literal_signature(&items[0], ctx) {
                                        let method = inner.name().as_slice().to_vec();
                                        let key_source = ctx.text(items[0].span()).to_vec();
                                        let line = ctx.line_col(node.span().start).line;
                                        self.indexed.push(IndexedEntry {
                                            method,
                                            key_sig: sig,
                                            key_source,
                                            span: node.span(),
                                            line,
                                        });
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
        let mut groups: Vec<Vec<&AssignmentEntry>> = Vec::new();
        for entry in &self.assignments {
            if let Some(group) = groups.iter_mut().find(|g| g[0].method == entry.method) {
                group.push(entry);
            } else {
                groups.push(vec![entry]);
            }
        }
        for group in &groups {
            if group.len() < 2 {
                continue;
            }
            let first_line = group[0].line;
            for entry in &group[1..] {
                let method = String::from_utf8_lossy(&entry.method);
                let msg = format!("{MSG_PREFIX}{method}{MSG_MIDDLE}{first_line}{MSG_SUFFIX}");
                ctx.report(&Self::META, entry.span, msg);
            }
        }

        let mut igroups: Vec<Vec<&IndexedEntry>> = Vec::new();
        for entry in &self.indexed {
            if let Some(group) = igroups
                .iter_mut()
                .find(|g| g[0].method == entry.method && g[0].key_sig == entry.key_sig)
            {
                group.push(entry);
            } else {
                igroups.push(vec![entry]);
            }
        }
        for group in &igroups {
            if group.len() < 2 {
                continue;
            }
            let first_line = group[0].line;
            for entry in &group[1..] {
                let method = String::from_utf8_lossy(&entry.method);
                let key = String::from_utf8_lossy(&entry.key_source);
                let msg =
                    format!("{MSG_PREFIX}{method}[{key}]={MSG_MIDDLE}{first_line}{MSG_SUFFIX}");
                ctx.report(&Self::META, entry.span, msg);
            }
        }
    }
}
