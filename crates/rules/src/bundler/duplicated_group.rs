//! `Bundler/DuplicatedGroup`, ported from RuboCop's
//! `lib/rubocop/cop/bundler/duplicated_group.rb`.
//!
//! # Whole-file, order-independent grouping
//!
//! Upstream's `on_new_investigation` collects every `(send nil? :group ...)`
//! node anywhere in the file via `def_node_search`, groups them by a
//! computed key, then reports every node after the first in each group whose
//! key repeats. This port mirrors that two-phase shape directly: `enter`
//! (subscribed to `CallNode` only) appends one [`GroupCall`] per matching
//! `group` send to `self.groups`, in source order; [`Rule::file_end`] then
//! groups those by key and reports every entry after the first survivor per
//! key. `Include: ['**/Gemfile']` restricts this cop to Gemfiles upstream;
//! the fixture harness (`crates/rules/tests/fixtures.rs`) builds its
//! `RuleSet` directly and never consults a cop's `Include`/`Exclude`
//! patterns (only the real CLI's file-discovery layer in `crates/cli` does,
//! via `LoadedConfig::is_cop_targeting`), so this rule enforces
//! `**/Gemfile` itself in [`Rule::file_start`]: that pattern matches only a
//! file whose basename is literally `Gemfile`, so a basename comparison is
//! exact, not an approximation.
//!
//! # Recovering the `send`-only span
//!
//! Upstream's flagged node is the inner `send` (whitequark wraps a
//! block-owning call in a separate `block` node, so the `send` node's own
//! `loc.expression` never includes `do ... end`), and the offense range is
//! further clipped to that node's own line
//! (`node.loc.column...node.loc.last_column` against `node.first_line`). A
//! Prism `CallNode`'s span always extends through its own attached block, so
//! [`ruby_ast::ext::call_span_excluding_block`] recovers the same
//! `send`-only range; since every `group` call's own arguments always fit on
//! their own opening line (the block's `do`/`{` immediately follows), that
//! range never itself spans multiple lines, making the extra per-line clip
//! upstream applies redundant here.
//!
//! # Finding the enclosing `source`/`git`/`platforms`/`path` block
//!
//! `find_source_key` walks `node.each_ancestor(:block)` outward from the
//! `group` send, returning the *first* block ancestor whose owning call's
//! method name is one of `SOURCE_BLOCK_NAMES` -- skipping any intervening
//! block that does not match, not just testing the nearest block ancestor.
//! [`DuplicatedGroup::block_owner_stack`] reproduces this without walking
//! ancestors on every `group` send: every `CallNode` that owns a literal
//! block (any name, mirroring `Lint/ConstantDefinitionInBlock`'s own
//! ancestor stack) pushes its message span and first-argument span on
//! `enter` and pops on `leave`, so the stack's current contents are exactly
//! the `group` node's block ancestors, innermost last. [`source_key`] then
//! scans it in reverse (innermost first) for the first entry whose message
//! text is one of the four names -- the same skip-non-matching walk as
//! upstream, at the cost of tracking non-matching block owners too (cheap:
//! two `u32`s per open block).
//!
//! # `group_attributes` vs. the message's `group_name`
//!
//! Upstream computes two different strings from the same argument list: the
//! grouping key folds a hash argument's pairs into one sorted,
//! comma-joined string and reduces any argument responding to `:value`
//! (`str`/`sym` literals; nothing else appears in a real Gemfile) to that
//! value's plain `to_s` -- so `:development` and `'development'` collapse to
//! the same key, and keyword-argument order/spelling differences do not
//! (each pair's own source differs) -- while the offense message's
//! `group_name` is simply every argument's own verbatim source joined by
//! `", "`, unnormalized. [`argument_attribute`] and [`group_name`] port
//! those two computations separately rather than deriving one from the
//! other. A `KeywordHashNode` (bare trailing `foo: true, bar: true`, no
//! braces) and an explicit `HashNode` (`{foo: true}`) are both whitequark's
//! single `hash` node shape and are treated identically here.

use std::collections::HashMap;

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::{CallNode, NodeList};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `SOURCE_BLOCK_NAMES`.
const SOURCE_BLOCK_NAMES: [&[u8]; 4] = [b"source", b"git", b"platforms", b"path"];

/// One `CallNode` currently open on the enclosing-block stack: its owning
/// method's name span (for [`source_key`]'s name comparison) and its first
/// argument's span, if it has one (RuboCop's
/// `send_node.first_argument&.source`).
#[derive(Debug, Clone, Copy)]
struct BlockOwner {
    name: Span,
    first_arg: Option<Span>,
}

/// One `group` call found anywhere in the file: its offense span, the
/// 1-based line it starts on (for the "already defined on line N" message),
/// its grouping key, and its message's verbatim `group_name`.
#[derive(Debug, Clone)]
struct GroupCall {
    span: Span,
    line: u32,
    key: String,
    group_name: String,
}

/// Whether `node` (a `CallNode`) owns a literal block, as opposed to a
/// `&block` argument pass (a `BlockArgumentNode`, not a `BlockNode`).
fn owns_literal_block(call: &CallNode<'_>) -> bool {
    call.block().is_some_and(|b| b.as_block_node().is_some())
}

/// One hash-typed argument's pairs, each reduced to its own verbatim source
/// text, sorted and comma-joined -- RuboCop's
/// `argument.pairs.map(&:source).sort.join(', ')`.
fn hash_pairs_attribute(ctx: &Context<'_>, elements: &NodeList<'_>) -> String {
    let mut pairs: Vec<String> = elements
        .iter()
        .map(|el| String::from_utf8_lossy(ctx.text(el.span())).into_owned())
        .collect();
    pairs.sort_unstable();
    pairs.join(", ")
}

/// One `group` argument's contribution to the grouping key: a hash
/// argument's sorted pairs, a `str`/`sym` literal's plain value (stripped of
/// quotes/colon, matching `value.to_s`), or any other argument's own
/// verbatim source -- RuboCop's `group_attributes` block.
fn argument_attribute(ctx: &Context<'_>, arg: &Node<'_>) -> String {
    if let Some(hash) = arg.as_hash_node() {
        return hash_pairs_attribute(ctx, &hash.elements());
    }
    if let Some(hash) = arg.as_keyword_hash_node() {
        return hash_pairs_attribute(ctx, &hash.elements());
    }
    if let Some(sym) = arg.as_symbol_node() {
        return String::from_utf8_lossy(sym.unescaped()).into_owned();
    }
    if let Some(s) = arg.as_string_node() {
        return String::from_utf8_lossy(s.unescaped()).into_owned();
    }
    String::from_utf8_lossy(ctx.text(arg.span())).into_owned()
}

/// The grouping key for one `group` call: `source_key` (from the nearest
/// matching enclosing block, or empty) followed by every argument's
/// [`argument_attribute`], sorted and concatenated with no separator --
/// RuboCop's `"#{source_key}#{group_attributes(node).sort.join}"` (note
/// `Array#join` with no argument joins with the empty string).
fn group_key(ctx: &Context<'_>, call: &CallNode<'_>, source_key: &str) -> String {
    let mut key = source_key.to_string();
    let Some(args) = call.arguments() else { return key };
    let mut attrs: Vec<String> =
        args.arguments().iter().map(|arg| argument_attribute(ctx, &arg)).collect();
    attrs.sort_unstable();
    for attr in attrs {
        key.push_str(&attr);
    }
    key
}

/// The offense message's `group_name`: every argument's own verbatim source,
/// joined by `", "` -- RuboCop's `node.arguments.map(&:source).join(', ')`.
fn group_name(ctx: &Context<'_>, call: &CallNode<'_>) -> String {
    let Some(args) = call.arguments() else { return String::new() };
    let mut name = String::new();
    for (i, arg) in args.arguments().iter().enumerate() {
        if i > 0 {
            name.push_str(", ");
        }
        name.push_str(&String::from_utf8_lossy(ctx.text(arg.span())));
    }
    name
}

/// RuboCop's `find_source_key`: the nearest enclosing block ancestor (see
/// [`DuplicatedGroup::block_owner_stack`]) whose owning method is one of
/// `SOURCE_BLOCK_NAMES`, formatted as `"#{method_name}#{first_argument&.source}"`
/// (empty when no such ancestor exists, or when it has no first argument).
fn source_key(ctx: &Context<'_>, stack: &[BlockOwner]) -> String {
    for owner in stack.iter().rev() {
        let name = ctx.text(owner.name);
        if SOURCE_BLOCK_NAMES.contains(&name) {
            let mut key = String::from_utf8_lossy(name).into_owned();
            if let Some(arg) = owner.first_arg {
                key.push_str(&String::from_utf8_lossy(ctx.text(arg)));
            }
            return key;
        }
    }
    String::new()
}

/// A Gem group, or a set of groups, should be listed only once in a
/// Gemfile.
///
/// For example, if the values of `source`, `git`, `platforms`, or `path`
/// surrounding `group` are different, no offense will be registered.
///
/// # Examples
///
/// ```ruby
/// # bad
/// group :development do
///   gem 'rubocop'
/// end
///
/// group :development do
///   gem 'rubocop-rails'
/// end
///
/// # bad (same set of groups declared twice)
/// group :development do
///   gem 'rubocop'
/// end
///
/// group :test, :development do
///   gem 'rspec'
/// end
///
/// # good
/// group :development do
///   gem 'rubocop'
/// end
///
/// group :development, :test do
///   gem 'rspec'
/// end
///
/// # good
/// gem 'rubocop', groups: [:development, :test]
/// gem 'rspec', groups: [:development, :test]
/// ```
#[derive(Debug, Clone, Default)]
pub struct DuplicatedGroup {
    /// Whether the current file's basename is literally `Gemfile`,
    /// matching upstream's `Include: ['**/Gemfile']` restriction (see the
    /// module doc).
    active: bool,
    block_owner_stack: Vec<BlockOwner>,
    groups: Vec<GroupCall>,
}

impl Rule for DuplicatedGroup {
    const META: RuleMeta = RuleMeta {
        name: "Bundler/DuplicatedGroup",
        department: Department::Bundler,
        summary: "Checks for duplicate gem group entries in Gemfile.",
        explanation: "\
A Gem group, or a set of groups, should be listed only once in a Gemfile.

For example, if the values of `source`, `git`, `platforms`, or `path`
surrounding `group` are different, no offense will be registered:

```ruby
platforms :ruby do
  group :default do
    gem 'openssl'
  end
end

platforms :jruby do
  group :default do
    gem 'jruby-openssl'
  end
end
```

```ruby
# bad
group :development do
  gem 'rubocop'
end

group :development do
  gem 'rubocop-rails'
end

# bad (same set of groups declared twice)
group :development do
  gem 'rubocop'
end

group :test, :development do
  gem 'rspec'
end

# good
group :development do
  gem 'rubocop'
end

group :development, :test do
  gem 'rspec'
end

# good
gem 'rubocop', groups: [:development, :test]
gem 'rspec', groups: [:development, :test]
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
`node.arguments.map(&:source)`/`argument.pairs.map(&:source)`'s equality-by-source-text and
`argument.value.to_s`'s literal-value comparisons are ported as direct byte-slice/`String`
comparisons rather than RuboCop's true structural `Node#==`/`Object#==`; every fixture's
arguments are bare symbol/string/splat/keyword literals, for which this is exact. `find_source_key`
only recognizes `source`/`git`/`platforms`/`path` ancestors regardless of receiver, matching
upstream's own `method_name`-only check (so `Foo.git(...) do ... end` counts too, on both sides of
this port).",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.active = ctx
            .source()
            .path()
            .file_name()
            .is_some_and(|name| name == std::ffi::OsStr::new("Gemfile"));
        self.block_owner_stack.clear();
        self.groups.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.active {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if owns_literal_block(&call) {
            let name = call.message_loc().map_or_else(|| node.span(), |loc| loc.span());
            let first_arg = call.arguments().and_then(|a| a.arguments().first()).map(|a| a.span());
            self.block_owner_stack.push(BlockOwner { name, first_arg });
        }
        if call.receiver().is_some() || call.name().as_slice() != b"group" {
            return;
        }
        let key = source_key(ctx, &self.block_owner_stack);
        self.groups.push(GroupCall {
            span: call_span_excluding_block(&call),
            line: ctx.line_col(node.span().start).line,
            key: group_key(ctx, &call, &key),
            group_name: group_name(ctx, &call),
        });
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if let Some(call) = node.as_call_node() {
            if owns_literal_block(&call) {
                self.block_owner_stack.pop();
            }
        }
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let mut by_key: HashMap<&str, Vec<usize>> = HashMap::new();
        for (i, group) in self.groups.iter().enumerate() {
            by_key.entry(group.key.as_str()).or_default().push(i);
        }
        for indices in by_key.values() {
            if indices.len() < 2 {
                continue;
            }
            let first_line = self.groups[indices[0]].line;
            for &i in &indices[1..] {
                let group = &self.groups[i];
                ctx.report(
                    &Self::META,
                    group.span,
                    format!(
                        "Gem group `{}` already defined on line {first_line} of the Gemfile.",
                        group.group_name
                    ),
                );
            }
        }
    }
}
