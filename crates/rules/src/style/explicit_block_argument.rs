//! `Style/ExplicitBlockArgument`, ported from RuboCop's
//! `lib/rubocop/cop/style/explicit_block_argument.rb`.
//!
//! # Detection shape
//!
//! Upstream matches `on_yield` against `(block $_ (args $...) (yield
//! $...))`: a literal block whose *entire* body is one bare `yield` call.
//! Prism always wraps a block body in a [`ruby_ast::node::StatementsNode`]
//! (even a single-statement one), so the Prism shape is "the block's body is
//! a `StatementsNode` holding exactly one statement, and that statement is a
//! `YieldNode`" ([`single_yield`]).
//!
//! Prism attaches a literal block to its owning
//! [`ruby_ast::node::CallNode`]/[`ruby_ast::node::SuperNode`]/
//! [`ruby_ast::node::ForwardingSuperNode`] via a `block` field (unlike
//! whitequark, which nests the `send`/`super`/`zsuper` node *inside* its
//! `block` wrapper), so this rule subscribes to those three kinds directly
//! and reads their own `block()` field -- no ancestor lookup needed to find
//! "the call this block belongs to" (see `Style/EmptyBlock`, ported the same
//! way). `block()` also yields a `BlockArgumentNode` for a `&blk` pass
//! (`foo(&blk)`); `.as_block_node()` filters that out, matching upstream
//! never matching a `(send ...)`-without-a-nested-`block` shape.
//!
//! The block's parameter list, when present, is only a match if it is an
//! ordinary `BlockParametersNode` (`|i, j|`) -- a numbered- (`_1`) or
//! `it`-parameter block is represented as the very same Prism `BlockNode`
//! (see `Lint/ConstantDefinitionInBlock`'s module doc), but whitequark parses
//! those as distinct `numblock`/`itblock` node types that never match
//! upstream's `block`-only pattern, so [`block_arg_names`] bails out (no
//! offense) rather than treating them as a zero-argument block.
//!
//! # The enclosing `def`
//!
//! Finding "the nearest enclosing method definition" needs a real ancestor
//! walk (a `DefNode` is never reachable from a `CallNode` through a forward
//! field, unlike the call/block relationship above). [`ExplicitBlockArgument`]
//! keeps its own stack of [`DefFacts`], pushed in `enter`/popped in `leave`
//! for every `DefNode`; its top is exactly RuboCop's
//! `block_node.each_ancestor(:any_def).first` (innermost first, so nested
//! `def`s resolve to the inner one -- see the "nested method definitions"
//! fixture). Only plain, POD facts are stored (spans, an owned block-name
//! string, an edit shape, and forwarded-argument texts for `zsuper`), never
//! borrowed `Node`s: [`Rule`] requires `Self: 'static`, and a node reference
//! cannot outlive the file whose arena it borrows from (see
//! `Lint::ConstantDefinitionInBlock`'s module doc for the same constraint).
//!
//! # One correction, up to three edits
//!
//! Each offense's [`Fix`] removes the block's body (from the end of the
//! owning call/`super`/`zsuper`, excluding the block, through the end of the
//! whole node -- a `CallNode`/`SuperNode`/`ForwardingSuperNode`'s own span
//! always extends through its attached block), rewrites that call to pass
//! `&block_name` instead, and -- *only the first time* a given `def` is seen
//! across the whole file -- also adds `&block_name` to that `def`'s own
//! parameter list. That third edit is upstream's `@def_nodes.add?(def_node)`
//! memo, reproduced here as `added: Vec<Span>` (keyed by the `def`'s span)
//! since two offenses inside the same method must not both try to insert
//! the parameter (the second [`Fix`]'s edits would collide with the first's
//! at the engine level, and would double the signature besides).
//!
//! # `zsuper`'s forwarded arguments
//!
//! `super { yield }` (no parens: Prism's `ForwardingSuperNode`) forwards
//! every one of the enclosing `def`'s own parameters verbatim, plus
//! `&block_name`, exactly like a bare `super` would already do implicitly --
//! upstream's `build_new_arguments_for_zsuper` reads each parameter's own
//! source text, except a positional optional parameter (`y = 42`), whose
//! *name* only is used (dropping the default); a keyword-optional parameter
//! (`y: 42`) is, true to the Ruby source this ports, *not* special-cased the
//! same way, so its default expression is forwarded literally too. Computed
//! once per `def`, in [`DefFacts::zsuper_args`], alongside the rest of that
//! `def`'s facts.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockNode, CallNode, DefNode, SuperNode};
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str =
    "Consider using explicit block argument in the surrounding method's signature over `yield`.";

/// A `BlockNode` with a body of exactly one `YieldNode` statement --
/// upstream's `(block $_ (args $...) (yield $...))` node-pattern match,
/// minus the block-owner and captures (read separately by the caller).
fn single_yield<'pr>(block: &BlockNode<'pr>) -> Option<ruby_ast::node::YieldNode<'pr>> {
    let stmts = block.body()?.as_statements_node()?;
    let body = stmts.body();
    if body.len() != 1 {
        return None;
    }
    body.first()?.as_yield_node()
}

/// A parameter node's declared name, for the naive identifier comparison
/// [`yielding_arguments`] needs -- `nil` for an anonymous `*`/`**`/`&` and
/// for any parameter shape with no single name (a destructured `(a, b)`
/// `MultiTargetNode`, or `...` forwarding), both of which then never equal
/// any yielded local variable's name, exactly like upstream's generic
/// `children.first` comparison landing on a non-symbol for the same shapes.
fn param_name(node: Node<'_>) -> Option<&[u8]> {
    if let Some(n) = node.as_required_parameter_node() {
        return Some(n.name().as_slice());
    }
    if let Some(n) = node.as_optional_parameter_node() {
        return Some(n.name().as_slice());
    }
    if let Some(n) = node.as_rest_parameter_node() {
        return n.name().map(|c| c.as_slice());
    }
    if let Some(n) = node.as_required_keyword_parameter_node() {
        return Some(n.name().as_slice());
    }
    if let Some(n) = node.as_optional_keyword_parameter_node() {
        return Some(n.name().as_slice());
    }
    if let Some(n) = node.as_keyword_rest_parameter_node() {
        return n.name().map(|c| c.as_slice());
    }
    if let Some(n) = node.as_block_parameter_node() {
        return n.name().map(|c| c.as_slice());
    }
    None
}

/// Visits every parameter of a `ParametersNode` (shared by a `def` and a
/// block's own `|...|` list) in source order: requireds, optionals, a rest,
/// posts, keywords, a keyword-rest, then a block parameter -- exactly the
/// order Ruby's grammar requires them to appear in, so this reproduces
/// whitequark's single flat `args` node's children without needing one.
fn for_each_param<'pr>(params: &ruby_ast::node::ParametersNode<'pr>, mut f: impl FnMut(Node<'pr>)) {
    for n in &params.requireds() {
        f(n);
    }
    for n in &params.optionals() {
        f(n);
    }
    if let Some(r) = params.rest() {
        f(r);
    }
    for n in &params.posts() {
        f(n);
    }
    for n in &params.keywords() {
        f(n);
    }
    if let Some(kr) = params.keyword_rest() {
        f(kr);
    }
    if let Some(b) = params.block() {
        f(b.as_node());
    }
}

/// The block's own declared parameter names, in source order -- `None` when
/// the block's parameter list is not an ordinary `BlockParametersNode` (a
/// numbered- or `it`-parameter block; see the module doc), matching
/// upstream never matching such a block at all. `Some(vec![])` covers both
/// a bare block (`{ }`, no `parameters()` at all) and an explicit empty one
/// (`{ || }`).
fn block_arg_names<'pr>(block: &BlockNode<'pr>) -> Option<Vec<Option<&'pr [u8]>>> {
    let Some(params_node) = block.parameters() else { return Some(Vec::new()) };
    let block_params = params_node.as_block_parameters_node()?;
    let mut names = Vec::new();
    if let Some(params) = block_params.parameters() {
        for_each_param(&params, |n| names.push(param_name(n)));
    }
    Some(names)
}

/// The `yield`'s own argument names, in order -- `None` (never matching any
/// parameter name) for any argument that is not a bare local variable read,
/// same as upstream's generic `children.first` comparison landing on
/// something other than a symbol for any other expression shape.
fn yield_arg_names<'pr>(node: &ruby_ast::node::YieldNode<'pr>) -> Vec<Option<&'pr [u8]>> {
    let Some(args) = node.arguments() else { return Vec::new() };
    args.arguments()
        .iter()
        .map(|n| n.as_local_variable_read_node().map(|lv| lv.name().as_slice()))
        .collect()
}

/// RuboCop's `yielding_arguments?`: the block's parameters and the
/// `yield`'s arguments must be the same length and equal pairwise by name.
/// (Upstream pads the shorter side with `nil` before zipping, but a `nil`
/// paired with a real name always fails its own `next false unless
/// yield_arg && block_arg` guard, so an outright length mismatch is
/// equivalent to -- and simpler than -- reproducing that padding.)
fn yielding_arguments(block_args: &[Option<&[u8]>], yield_args: &[Option<&[u8]>]) -> bool {
    block_args.len() == yield_args.len()
        && block_args
            .iter()
            .zip(yield_args)
            .all(|(b, y)| matches!((b, y), (Some(b), Some(y)) if b == y))
}

/// How to add `&block_name` to a `def`'s own parameter list -- computed once
/// when the `def` is entered, from facts that never change afterwards.
#[derive(Debug, Clone, Copy)]
enum DefEdit {
    /// No parentheses at all (`def m`): insert right after the method name.
    NoParens { insert_at: u32 },
    /// Parentheses with zero parameters (`def m()`): replace them outright.
    EmptyParens { parens: Span },
    /// At least one parameter: extend the last one, unless it is already a
    /// block parameter (`is_blockarg`), matching upstream's `unless
    /// last_arg.blockarg_type?`.
    HasArgs { last_arg_end: u32, is_blockarg: bool },
}

/// Everything a `def` needs, computed once on `enter` and read back by every
/// offending call/`super`/`zsuper` found in its body. Plain owned data only
/// -- see the module doc's "The enclosing `def`" section for why.
#[derive(Debug, Clone)]
struct DefFacts {
    /// This `def`'s own span: the de-duplication key in
    /// [`ExplicitBlockArgument::added`].
    span: Span,
    /// RuboCop's `extract_block_name`: the existing block parameter's name,
    /// or the literal `"block"` when the `def` has none yet.
    block_name: Box<str>,
    /// How to add the block parameter, the first time it is needed.
    edit: DefEdit,
    /// RuboCop's `build_new_arguments_for_zsuper`, minus the trailing
    /// `&block_name` (appended by the caller): every parameter's own source
    /// text, except a positional optional parameter's bare name.
    zsuper_args: Vec<Box<str>>,
}

/// Builds [`DefFacts`] for a just-entered `DefNode`.
fn build_def_facts(def_node: &DefNode<'_>, full_span: Span, ctx: &Context<'_>) -> DefFacts {
    let Some(params) = def_node.parameters() else {
        let edit = match (def_node.lparen_loc(), def_node.rparen_loc()) {
            (Some(open), Some(close)) => {
                DefEdit::EmptyParens { parens: Span::new(open.span().start, close.span().end) }
            }
            _ => DefEdit::NoParens { insert_at: def_node.name_loc().span().end },
        };
        return DefFacts {
            span: full_span,
            block_name: Box::from("block"),
            edit,
            zsuper_args: Vec::new(),
        };
    };

    let mut last: Option<Node<'_>> = None;
    let mut zsuper_args = Vec::new();
    for_each_param(&params, |n| {
        if n.as_block_parameter_node().is_none() {
            let text = if let Some(opt) = n.as_optional_parameter_node() {
                String::from_utf8_lossy(opt.name().as_slice()).into_owned()
            } else {
                String::from_utf8_lossy(ctx.text(n.span())).into_owned()
            };
            zsuper_args.push(Box::from(text));
        }
        last = Some(n);
    });

    let (block_name, is_blockarg) = match last.and_then(|n| n.as_block_parameter_node()) {
        Some(b) => (
            b.name().map_or_else(
                || "block".to_string(),
                |id| String::from_utf8_lossy(id.as_slice()).into_owned(),
            ),
            true,
        ),
        None => ("block".to_string(), false),
    };
    let last_arg_end = last.map_or_else(|| params.as_node().span().end, |n| n.span().end);

    DefFacts {
        span: full_span,
        block_name: Box::from(block_name),
        edit: DefEdit::HasArgs { last_arg_end, is_blockarg },
        zsuper_args,
    }
}

/// RuboCop's `insert_argument`: extends `last_arg_end` over an immediately
/// adjacent trailing comma (RuboCop's `range_with_surrounding_comma(...,
/// :right)`, which only ever absorbs a directly-touching comma, no
/// intervening whitespace) and inserts `&block_name` after it, adding a
/// leading comma of its own only if one was not already there to absorb.
fn insert_argument_edit(ctx: &Context<'_>, last_arg_end: u32, block_name: &str) -> Edit {
    let bytes = ctx.source().bytes();
    let mut end = last_arg_end as usize;
    while end < bytes.len() && bytes[end] == b',' {
        end += 1;
    }
    let end = u32::try_from(end).unwrap_or(last_arg_end);
    let text =
        if end > last_arg_end { format!(" &{block_name}") } else { format!(", &{block_name}") };
    Edit::insert(end, text.into_bytes())
}

/// The edit that adds `&block_name` to `def_facts`'s own `def`, if it has
/// not already gained one (from an earlier offense in the same `def`, or --
/// `is_blockarg` -- because it already declares one).
fn def_add_edit(def_facts: &DefFacts, ctx: &Context<'_>) -> Option<Edit> {
    match def_facts.edit {
        DefEdit::HasArgs { is_blockarg: true, .. } => None,
        DefEdit::NoParens { insert_at } => {
            Some(Edit::insert(insert_at, format!("(&{})", def_facts.block_name).into_bytes()))
        }
        DefEdit::EmptyParens { parens } => {
            Some(Edit::replace(parens, format!("(&{})", def_facts.block_name).into_bytes()))
        }
        DefEdit::HasArgs { last_arg_end, is_blockarg: false } => {
            Some(insert_argument_edit(ctx, last_arg_end, &def_facts.block_name))
        }
    }
}

/// RuboCop's `add_block_argument(send_node, ...)`/`correct_call_node` for a
/// `CallNode`: extend the last real argument, replace an empty `()`
/// outright (simpler than upstream's insert-then-remove-the-old-parens
/// dance for the same net text), or insert `(&block_name)` right after the
/// call for a bare, parenthesis-free dispatch.
fn call_add_edit(call: &CallNode<'_>, block_name: &str, ctx: &Context<'_>) -> Edit {
    if let Some(args) = call.arguments().filter(|a| !a.arguments().is_empty()) {
        let last = args.arguments().last().expect("checked non-empty above");
        insert_argument_edit(ctx, last.span().end, block_name)
    } else if let (Some(open), Some(close)) = (call.opening_loc(), call.closing_loc()) {
        Edit::replace(
            Span::new(open.span().start, close.span().end),
            format!("(&{block_name})").into_bytes(),
        )
    } else {
        Edit::insert(
            ext::call_span_excluding_block(call).end,
            format!("(&{block_name})").into_bytes(),
        )
    }
}

/// `call`'s own span, excluding any attached block -- also where the
/// offense's block-body-removal edit begins.
fn call_removal_start(call: &CallNode<'_>) -> u32 {
    ext::call_span_excluding_block(call).end
}

/// `SuperNode`'s analog of [`ext::call_span_excluding_block`]: `SuperNode`
/// has no shared mixin with `CallNode` in Prism, so its own span's
/// block-excluded end is recomputed the same way `Layout/RescueEnsureAlignment`
/// does.
fn super_removal_start(sup: &SuperNode<'_>) -> u32 {
    if let Some(rparen) = sup.rparen_loc() {
        return rparen.span().end;
    }
    if let Some(args) = sup.arguments() {
        if let Some(last) = args.arguments().last() {
            return last.span().end;
        }
    }
    sup.keyword_loc().span().end
}

/// RuboCop's `add_block_argument(send_node, ...)`/`correct_call_node` for a
/// `SuperNode` (`super`/`super(...)`, with explicit parentheses or real
/// arguments -- `zsuper`/`super { }` is a distinct `ForwardingSuperNode`,
/// handled by [`zsuper_add_edit`]).
fn super_add_edit(sup: &SuperNode<'_>, block_name: &str, ctx: &Context<'_>) -> Edit {
    if let Some(args) = sup.arguments().filter(|a| !a.arguments().is_empty()) {
        let last = args.arguments().last().expect("checked non-empty above");
        insert_argument_edit(ctx, last.span().end, block_name)
    } else if let (Some(open), Some(close)) = (sup.lparen_loc(), sup.rparen_loc()) {
        Edit::replace(
            Span::new(open.span().start, close.span().end),
            format!("(&{block_name})").into_bytes(),
        )
    } else {
        Edit::insert(super_removal_start(sup), format!("(&{block_name})").into_bytes())
    }
}

/// RuboCop's `build_new_arguments_for_zsuper` plus the trailing
/// `&block_name`, joined and parenthesized, for a bare `super { yield }`.
fn zsuper_add_edit(insert_at: u32, def_facts: &DefFacts, block_name: &str) -> Edit {
    let mut parts: Vec<&str> =
        def_facts.zsuper_args.iter().map(std::convert::AsRef::as_ref).collect();
    let block_arg = format!("&{block_name}");
    parts.push(&block_arg);
    Edit::insert(insert_at, format!("({})", parts.join(", ")).into_bytes())
}

/// Consider using explicit block argument to avoid writing block literal that just passes its arguments to another block.
#[derive(Debug, Clone)]
pub struct ExplicitBlockArgument {
    /// Facts about every `def` currently open, outermost first -- see the
    /// module doc's "The enclosing `def`" section.
    def_stack: Vec<DefFacts>,
    /// Spans of every `def` that has already gained (or been found to
    /// already have) its `&block_name` parameter -- RuboCop's
    /// `@def_nodes.add?(def_node)` memo, keyed by span since a `Node`
    /// itself cannot be stored (see the module doc).
    added: Vec<Span>,
}

impl ExplicitBlockArgument {
    /// RuboCop's `yielding_block?` plus its `yielding_arguments?` guard: a
    /// literal block whose sole statement is a `yield` whose arguments are
    /// exactly the block's own parameters, in order and by name.
    fn matches(block: &BlockNode<'_>) -> bool {
        let Some(yield_node) = single_yield(block) else { return false };
        let Some(block_args) = block_arg_names(block) else { return false };
        let yield_args = yield_arg_names(&yield_node);
        yielding_arguments(&block_args, &yield_args)
    }

    /// Reports the offense and builds its `Fix`: remove the block's body,
    /// rewrite the call to pass `&block_name`, and -- the first time this
    /// `def` is seen -- add `&block_name` to its own parameter list too.
    fn report(
        &mut self,
        ctx: &mut Context<'_>,
        full_span: Span,
        removal_start: u32,
        call_edit: Edit,
        def_facts: &DefFacts,
    ) {
        let mut edits = vec![Edit::delete(Span::new(removal_start, full_span.end)), call_edit];
        if !self.added.contains(&def_facts.span) {
            if let Some(def_edit) = def_add_edit(def_facts, ctx) {
                edits.push(def_edit);
            }
            self.added.push(def_facts.span);
        }
        let fix = Fix { applicability: Applicability::Safe, edits };
        ctx.report_with_fix(&Self::META, full_span, MSG, fix);
    }

    fn check_call(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
        if !Self::matches(&block) {
            return;
        }
        let Some(def_facts) = self.def_stack.last().cloned() else { return };
        let call_edit = call_add_edit(&call, &def_facts.block_name, ctx);
        let removal_start = call_removal_start(&call);
        self.report(ctx, node.span(), removal_start, call_edit, &def_facts);
    }

    fn check_super(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let sup = node.as_super_node().expect("kind matched");
        let Some(block) = sup.block().and_then(|b| b.as_block_node()) else { return };
        if !Self::matches(&block) {
            return;
        }
        let Some(def_facts) = self.def_stack.last().cloned() else { return };
        let call_edit = super_add_edit(&sup, &def_facts.block_name, ctx);
        let removal_start = super_removal_start(&sup);
        self.report(ctx, node.span(), removal_start, call_edit, &def_facts);
    }

    fn check_forwarding_super(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let sup = node.as_forwarding_super_node().expect("kind matched");
        let Some(block) = sup.block() else { return };
        if !Self::matches(&block) {
            return;
        }
        let Some(def_facts) = self.def_stack.last().cloned() else { return };
        // A bare `super` has no parentheses/arguments ever, so its own
        // span excluding the block always ends right after the keyword.
        let insert_at = node.span().start + 5;
        let call_edit = zsuper_add_edit(insert_at, &def_facts, &def_facts.block_name);
        self.report(ctx, node.span(), insert_at, call_edit, &def_facts);
    }
}

impl Rule for ExplicitBlockArgument {
    const META: RuleMeta = RuleMeta {
        name: "Style/ExplicitBlockArgument",
        department: Department::Style,
        summary: "Consider using explicit block argument to avoid writing block literal that just passes its arguments to another block.",
        explanation: "\
Enforces the use of explicit block argument to avoid writing
block literal that just passes its arguments to another block.

NOTE: This cop only registers an offense if the block args match the
yield args exactly.

```ruby
# bad
def with_tmp_dir
  Dir.mktmpdir do |tmp_dir|
    Dir.chdir(tmp_dir) { |dir| yield dir } # block just passes arguments
  end
end

# bad
def nine_times
  9.times { yield }
end

# good
def with_tmp_dir(&block)
  Dir.mktmpdir do |tmp_dir|
    Dir.chdir(tmp_dir, &block)
  end
end

with_tmp_dir do |dir|
  puts \"dir is accessible as a parameter and pwd is set: #{dir}\"
end

# good
def nine_times(&block)
  9.times(&block)
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::DefNode,
            NodeKind::CallNode,
            NodeKind::SuperNode,
            NodeKind::ForwardingSuperNode,
        ],
        config: &[],
        blind_spots: "\
The block/yield argument match is a naive by-name comparison (upstream's own
`children.first` equality), ported faithfully: a destructured block
parameter, `...` forwarding, or an anonymous `*`/`**`/`&` parameter never
matches any yielded expression, even when the intent is equivalent. A
`def`'s own anonymous block-forwarding parameter (`def m(&)`) is treated as
having no existing name, so its extracted block name falls back to the
literal `block` rather than reusing the anonymous form.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { def_stack: Vec::new(), added: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.def_stack.clear();
        self.added.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::DefNode { .. } => {
                let def_node = node.as_def_node().expect("kind matched");
                self.def_stack.push(build_def_facts(&def_node, node.span(), ctx));
            }
            Node::CallNode { .. } => self.check_call(node, ctx),
            Node::SuperNode { .. } => self.check_super(node, ctx),
            Node::ForwardingSuperNode { .. } => self.check_forwarding_super(node, ctx),
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if let Node::DefNode { .. } = node {
            self.def_stack.pop();
        }
    }
}
