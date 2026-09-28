//! `Lint/UselessMethodDefinition`, ported from RuboCop's
//! `lib/rubocop/cop/lint/useless_method_definition.rb`.
//!
//! # Prism shape
//!
//! Upstream drives off `on_def`/`on_defs` (whitequark distinguishes instance
//! vs. singleton defs); Prism has one `DefNode` for both, disambiguated only
//! by its `receiver` field, so a single traversal covers both aliases.
//!
//! `node.parent` for a def wrapped in an access-modifier call (`private def
//! foo; end`) is, in whitequark, the `send` node directly -- whitequark has
//! no wrapper for call arguments. Prism interposes an `ArgumentsNode` there,
//! so this rule walks the tree itself (rather than subscribing through
//! `META.kinds`) tracking a hand-rolled ancestor stack that skips
//! `ArgumentsNode`, mirroring the pattern in
//! `lint/ambiguous_regexp_literal.rs`.
//!
//! Whitequark elides a single-statement `begin`, so `node.body` for a method
//! containing just `super`/`super(...)` is that statement directly. Prism
//! always wraps a non-empty body in a `StatementsNode`, so `delegating?` is
//! reproduced here by unwrapping a `StatementsNode` with exactly one child.
//! Bare `super` (whitequark's `zsuper`) is Prism's `ForwardingSuperNode`;
//! `super(...)`/`super()` is `SuperNode`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{ArgumentsNode, CallNode, DefNode};
use ruby_ast::{walk, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

/// Upstream's `MSG`.
const MSG: &str = "Useless method definition detected.";

/// Checks for useless method definitions, specifically: empty constructors
/// and methods just delegating to `super`.
#[derive(Debug, Clone)]
pub struct UselessMethodDefinition;

impl Rule for UselessMethodDefinition {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UselessMethodDefinition",
        department: Department::Lint,
        summary: "Checks for useless method definitions, specifically: empty constructors and \
                   methods just delegating to `super`.",
        explanation: "\
```ruby
# bad
def initialize
  super
end

def method
  super
end

# good - with default arguments
def initialize(x = Object.new)
  super
end

# good
def initialize
  super
  initialize_internals
end

def method(*args)
  super(:extra_arg, *args)
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "\
This cop is unsafe: an empty constructor delegating to `super` can be intentionally overriding a \
parent constructor, which is bad on its own, but not what this cop reports.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let mut finder = Finder { stack: Vec::new(), candidates: Vec::new() };
        walk(&root, &mut finder);
        for candidate in finder.candidates {
            let delegates = match &candidate.delegation {
                Delegation::Bare => true,
                Delegation::Explicit { method_args, super_args } => {
                    method_args.len() == super_args.len()
                        && method_args
                            .iter()
                            .zip(super_args)
                            .all(|(m, s)| ctx.text(*m) == ctx.text(*s))
                }
            };
            if !delegates {
                continue;
            }
            let fix = Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::delete(candidate.remove_span)],
            };
            ctx.report_with_fix(&Self::META, candidate.report_span, MSG, fix);
        }
    }
}

/// What the def's single body statement delegates: RuboCop's `delegating?`.
enum Delegation {
    /// Bare `super` (whitequark's `zsuper`): always delegates.
    Bare,
    /// `super(...)`/`super()`: delegates only when its argument sources
    /// equal the def's own parameter sources, positionally.
    Explicit { method_args: Vec<Span>, super_args: Vec<Span> },
}

/// One useless definition found by [`Finder`]: where to report (the def
/// node's own span) and what byte range `corrector.remove` erases (the
/// wrapping access-modifier call's span when present, else the def itself).
struct Candidate {
    report_span: Span,
    remove_span: Span,
    delegation: Delegation,
}

/// Walks the whole tree collecting [`Candidate`]s, tracking a live ancestor
/// stack (needed for `node.parent`'s receiver/name, which
/// [`linter::context::NodeInfo`] does not carry) with Prism's
/// `ArgumentsNode` wrapper -- absent from whitequark, where a `send`'s
/// arguments are its own direct children -- skipped transparently.
struct Finder<'pr> {
    stack: Vec<Node<'pr>>,
    candidates: Vec<Candidate>,
}

impl<'pr> Visitor<'pr> for Finder<'pr> {
    fn enter(&mut self, node: &Node<'pr>) {
        if let Some(def) = node.as_def_node() {
            if let Some(candidate) = check_def(&self.stack, &def) {
                self.candidates.push(candidate);
            }
        }
        if node.kind() != NodeKind::ArgumentsNode {
            self.stack.push(*node);
        }
    }

    fn leave(&mut self, node: &Node<'pr>) {
        if node.kind() != NodeKind::ArgumentsNode {
            self.stack.pop();
        }
    }
}

/// RuboCop's `on_def`/`on_defs` body.
fn check_def<'pr>(stack: &[Node<'pr>], def: &DefNode<'pr>) -> Option<Candidate> {
    if method_definition_with_modifier(stack) || use_rest_or_optional_args(def) {
        return None;
    }
    let stmt = single_statement(def.body())?;
    // whitequark wraps `super do ... end`/`super { ... }` in a `block` node, which
    // is neither `zsuper` nor `super`; a `&blk` block-pass is one of `super`'s
    // arguments. Prism keeps both in the super node's `block`.
    let delegation = if let Some(zsuper) = stmt.as_forwarding_super_node() {
        if zsuper.block().is_some() {
            return None;
        }
        Delegation::Bare
    } else {
        let sup = stmt.as_super_node()?;
        let mut super_args = super_argument_spans(sup.arguments());
        if let Some(block) = sup.block() {
            block.as_block_argument_node()?;
            super_args.push(block.span());
        }
        Delegation::Explicit { method_args: def_argument_spans(def), super_args }
    };
    let modifier_call = stack.last().and_then(Node::as_call_node).filter(is_access_modifier);
    let remove_span = match modifier_call {
        Some(_) => stack.last().expect("matched above").span(),
        None => def.as_node().span(),
    };
    Some(Candidate { report_span: def.as_node().span(), remove_span, delegation })
}

/// RuboCop's `method_definition_with_modifier?`: the def's logical parent
/// (whitequark's `node.parent`, skipping Prism's `ArgumentsNode` wrapper) is
/// some other call -- one that isn't a `public`/`protected`/`private`/
/// `module_function` wrapper, so `def` is really a generic macro argument
/// (`do_something def method; end`) rather than a method definition proper.
fn method_definition_with_modifier(stack: &[Node<'_>]) -> bool {
    let Some(parent) = stack.last() else { return false };
    let Some(call) = parent.as_call_node() else { return false };
    !is_access_modifier(&call)
}

/// `rubocop-ast`'s `MethodDispatchNode#non_bare_access_modifier?` shape,
/// narrowed to what this cop needs: a receiver-less call to
/// `public`/`protected`/`private`/`module_function`. Every call reaching
/// this check already has the def node as an argument (it is that def's
/// logical parent), so the "has arguments" half of `non_bare_access_
/// modifier?` is automatically satisfied.
fn is_access_modifier(call: &CallNode<'_>) -> bool {
    call.receiver().is_none()
        && matches!(
            call.name().as_slice(),
            b"public" | b"protected" | b"private" | b"module_function"
        )
}

/// RuboCop's `use_rest_or_optional_args?`: any `*rest`/`x = default`/
/// `x: default`/`**kwrest` parameter (1.91's `%i[restarg optarg kwoptarg
/// kwrestarg]`); `&block`, `**nil` and `...` are not in that list.
fn use_rest_or_optional_args(def: &DefNode<'_>) -> bool {
    let Some(params) = def.parameters() else { return false };
    !params.optionals().is_empty()
        || params.rest().is_some()
        || params.keywords().iter().any(|k| k.kind() == NodeKind::OptionalKeywordParameterNode)
        || params.keyword_rest().is_some_and(|k| k.kind() == NodeKind::KeywordRestParameterNode)
}

/// RuboCop's `delegating?`'s implicit `node&.begin_type?` rejection: Prism
/// always wraps a non-empty body in a `StatementsNode` (whitequark elides it
/// for a single statement), so a single delegating statement is recovered by
/// unwrapping a one-element `StatementsNode`; any other shape (empty body,
/// multiple statements) means "not delegating" (`None`), same as upstream's
/// `node&.zsuper_type?`/`node&.super_type?` both failing on a `begin` node.
fn single_statement(body: Option<Node<'_>>) -> Option<Node<'_>> {
    let stmts = body?.as_statements_node()?;
    let items = stmts.body();
    if items.len() == 1 {
        items.first()
    } else {
        None
    }
}

/// The def's own parameters, in declaration order, as spans -- RuboCop's
/// `def_node.arguments.map(&:source)`. Only reached once
/// [`use_rest_or_optional_args`] has already ruled out rest/optional/
/// optional-keyword parameters, so in practice this is required positionals,
/// post-rest requireds (impossible without a rest, but harmless to include),
/// required keywords, `**kwrest`, and `&block`.
fn def_argument_spans(def: &DefNode<'_>) -> Vec<Span> {
    let Some(params) = def.parameters() else { return Vec::new() };
    let mut spans = Vec::new();
    for n in &params.requireds() {
        spans.push(n.span());
    }
    for n in &params.optionals() {
        spans.push(n.span());
    }
    if let Some(rest) = params.rest() {
        spans.push(rest.span());
    }
    for n in &params.posts() {
        spans.push(n.span());
    }
    for n in &params.keywords() {
        spans.push(n.span());
    }
    if let Some(kwrest) = params.keyword_rest() {
        spans.push(kwrest.span());
    }
    if let Some(block) = params.block() {
        spans.push(block.as_node().span());
    }
    spans
}

/// A `super(...)` call's own arguments, in source order -- RuboCop's
/// `node.arguments.map(&:source)`; `super()`/bare `super()` with no
/// `ArgumentsNode` at all yields an empty list.
fn super_argument_spans(args: Option<ArgumentsNode<'_>>) -> Vec<Span> {
    args.map(|a| a.arguments().iter().map(|n| n.span()).collect()).unwrap_or_default()
}
