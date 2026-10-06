//! `Rails/HasManyOrHasOneDependent`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/has_many_or_has_one_dependent.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, each_descendant, for_each_child};
use ruby_source::Span;

use super::util::parser_args;

const MSG: &str = "Specify a `:dependent` option.";

/// Define the dependent option to the `has_many` and `has_one` associations.
#[derive(Debug, Clone)]
pub struct HasManyOrHasOneDependent;

impl Rule for HasManyOrHasOneDependent {
    const META: RuleMeta = RuleMeta {
        name: "Rails/HasManyOrHasOneDependent",
        department: Department::Rails,
        summary: "Define the dependent option to the has_many and has_one associations.",
        explanation: "Looks for `has_many` or `has_one` associations that don't specify a \
                      `:dependent` option.\n\nIt doesn't register an offense if `:through` or \
                      `dependent: nil` is specified, or if the model is read-only.\n\n```ruby\n\
                      # bad\nclass User < ActiveRecord::Base\n  has_many :comments\n  has_one \
                      :avatar\nend\n\n# good\nclass User < ActiveRecord::Base\n  has_many \
                      :comments, dependent: :restrict_with_exception\n  has_one :avatar, \
                      dependent: :destroy\n  has_many :articles, dependent: nil\n  has_many \
                      :patients, through: :appointments\nend\n\nclass User < \
                      ActiveRecord::Base\n  has_many :comments\n  has_one :avatar\n\n  def \
                      readonly?\n    true\n  end\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let source = ctx.source().bytes();
        if !source.windows(7).any(|w| w == b"has_one") && !source.windows(8).any(|w| w == b"has_many")
        {
            return;
        }
        let root = ctx.parsed().root();
        let mut stack = Vec::new();
        let mut hits = Vec::new();
        walk(&root, &mut stack, &mut hits);
        for span in hits {
            ctx.report(&Self::META, span, MSG);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Plain,
    /// parser's `begin` (multiple statements, or parentheses)
    Begin,
    /// parser's `block`; the node is the call that owns the block
    Block,
}

#[derive(Clone, Copy)]
struct Frame<'pr> {
    node: Node<'pr>,
    kind: Kind,
}

/// Walks with a stack of parser-style ancestors (single-statement
/// `StatementsNode`s and `ArgumentsNode`s elided, a call with a literal block
/// being the `block` parent of its own `send`).
fn walk<'pr>(node: &Node<'pr>, stack: &mut Vec<Frame<'pr>>, hits: &mut Vec<Span>) {
    if let Some(statements) = node.as_statements_node() {
        let begin = statements.body().iter().count() > 1;
        if begin {
            stack.push(Frame { node: *node, kind: Kind::Begin });
        }
        for_each_child(node, |child| walk(child, stack, hits));
        if begin {
            stack.pop();
        }
        return;
    }
    if node.as_arguments_node().is_some() {
        for_each_child(node, |child| walk(child, stack, hits));
        return;
    }
    if let Some(call) = node.as_call_node() {
        let has_block = call.block().is_some_and(|b| b.as_block_node().is_some());
        if has_block {
            stack.push(Frame { node: *node, kind: Kind::Block });
        }
        if let Some(span) = check(&call, stack) {
            hits.push(span);
        }
        for_each_child(node, |child| {
            if child.as_block_node().is_some() {
                for_each_child(child, |inner| walk(inner, stack, hits));
            } else {
                stack.push(Frame { node: *node, kind: Kind::Plain });
                walk(child, stack, hits);
                stack.pop();
            }
        });
        if has_block {
            stack.pop();
        }
        return;
    }
    let kind = if node.as_parentheses_node().is_some() { Kind::Begin } else { Kind::Plain };
    stack.push(Frame { node: *node, kind });
    for_each_child(node, |child| walk(child, stack, hits));
    stack.pop();
}

fn check(call: &CallNode<'_>, stack: &[Frame<'_>]) -> Option<Span> {
    let name = call.name();
    if !matches!(name.as_slice(), b"has_many" | b"has_one") || call.is_safe_navigation() {
        return None;
    }
    let parent = stack.last();
    if parent.is_some_and(|p| active_resource(&p.node)) || readonly_model(parent) {
        return None;
    }
    let args = parser_args(call);
    let without_options = args.len() == 1;
    if !without_options {
        let options = args.last().and_then(hash_elements);
        if valid_options(options.as_deref()) {
            return None;
        }
    }
    if valid_options_in_with_options_block(stack) {
        return None;
    }
    call.message_loc().map(|l| l.span())
}

fn hash_elements<'pr>(node: &Node<'pr>) -> Option<Vec<Node<'pr>>> {
    if let Some(h) = node.as_hash_node() {
        Some(h.elements().iter().collect())
    } else {
        node.as_keyword_hash_node().map(|h| h.elements().iter().collect())
    }
}

/// `(const (const {nil? cbase} :ActiveResource) :Base)` anywhere in `node`.
fn active_resource(node: &Node<'_>) -> bool {
    let mut found = is_active_resource_base(node);
    if !found {
        each_descendant(node, &mut |n| found = found || is_active_resource_base(n));
    }
    found
}

fn is_active_resource_base(node: &Node<'_>) -> bool {
    let Some(path) = node.as_constant_path_node() else { return false };
    if path.name().is_none_or(|n| n.as_slice() != b"Base") {
        return false;
    }
    let Some(parent) = path.parent() else { return false };
    if let Some(read) = parent.as_constant_read_node() {
        read.name().as_slice() == b"ActiveResource"
    } else if let Some(inner) = parent.as_constant_path_node() {
        inner.parent().is_none() && inner.name().is_some_and(|n| n.as_slice() == b"ActiveResource")
    } else {
        false
    }
}

fn readonly_model(parent: Option<&Frame<'_>>) -> bool {
    let Some(parent) = parent else { return false };
    let mut found = false;
    each_descendant(&parent.node, &mut |n| found = found || is_readonly_def(n));
    found
}

/// `(def :readonly? (args) (true))`
fn is_readonly_def(node: &Node<'_>) -> bool {
    let Some(def) = node.as_def_node() else { return false };
    if def.name().as_slice() != b"readonly?" || def.receiver().is_some() || def.parameters().is_some()
    {
        return false;
    }
    let Some(body) = def.body() else { return false };
    let Some(statements) = body.as_statements_node() else { return false };
    let mut it = statements.body().iter();
    matches!((it.next(), it.next()), (Some(only), None) if only.as_true_node().is_some())
}

fn valid_options_in_with_options_block(stack: &[Frame<'_>]) -> bool {
    let Some(parent) = stack.last() else { return true };
    let last = stack.len() - 1;
    let index = if parent.kind == Kind::Begin || association_extension_block(parent) {
        last.checked_sub(1)
    } else {
        Some(last)
    };
    index.is_some_and(|i| contain_valid_options_in_with_options_block(stack, i))
}

fn contain_valid_options_in_with_options_block(stack: &[Frame<'_>], index: usize) -> bool {
    let Some(options) = with_options_block(&stack[index]) else { return false };
    if valid_options(Some(&options)) {
        return true;
    }
    index >= 2 && contain_valid_options_in_with_options_block(stack, index - 2)
}

/// Number of parameters of a plain (non-numbered, non-`it`) block.
fn block_param_count(call: &CallNode<'_>) -> Option<usize> {
    let block = call.block()?.as_block_node()?;
    let Some(params) = block.parameters() else { return Some(0) };
    let params = params.as_block_parameters_node()?;
    let mut count = params.locals().iter().count();
    if let Some(p) = params.parameters() {
        count += p.requireds().iter().count()
            + p.optionals().iter().count()
            + p.posts().iter().count()
            + p.keywords().iter().count()
            + usize::from(p.rest().is_some())
            + usize::from(p.keyword_rest().is_some())
            + usize::from(p.block().is_some());
    }
    Some(count)
}

/// `(block (send nil? :has_many _) (args) ...)`
fn association_extension_block(frame: &Frame<'_>) -> bool {
    if frame.kind != Kind::Block {
        return false;
    }
    let Some(call) = frame.node.as_call_node() else { return false };
    call.name().as_slice() == b"has_many"
        && call.receiver().is_none()
        && !call.is_safe_navigation()
        && call.arguments().is_some_and(|a| a.arguments().iter().count() == 1)
        && block_param_count(&call) == Some(0)
}

/// `(block (send nil? :with_options (hash $...)) (args _?) ...)`
fn with_options_block<'pr>(frame: &Frame<'pr>) -> Option<Vec<Node<'pr>>> {
    if frame.kind != Kind::Block {
        return None;
    }
    let call = frame.node.as_call_node()?;
    if call.name().as_slice() != b"with_options"
        || call.receiver().is_some()
        || call.is_safe_navigation()
        || block_param_count(&call).is_none_or(|n| n > 1)
    {
        return None;
    }
    let mut args = call.arguments()?.arguments().iter();
    let (Some(only), None) = (args.next(), args.next()) else { return None };
    hash_elements(&only)
}

fn valid_options(options: Option<&[Node<'_>]>) -> bool {
    let Some(options) = options else { return false };
    let Some(first) = options.first() else { return false };
    let pairs: Vec<Node<'_>>;
    let options = match first.as_assoc_splat_node().and_then(|s| s.value()) {
        Some(value) if value.as_hash_node().is_some() => {
            pairs = value
                .as_hash_node()
                .map(|h| h.elements().iter().filter(|e| e.as_assoc_node().is_some()).collect())
                .unwrap_or_default();
            &pairs[..]
        }
        _ => options,
    };
    options.iter().any(|o| {
        o.as_assoc_node().is_some_and(|pair| {
            pair.key().as_symbol_node().is_some_and(|k| match k.unescaped() {
                b"dependent" => true,
                b"through" => pair.value().as_nil_node().is_none(),
                _ => false,
            })
        })
    })
}
