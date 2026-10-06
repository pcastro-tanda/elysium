//! `Performance/RedundantMatch`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/redundant_match.rb`.
//!
//! `node.value_used?` (rubocop-ast) is re-created in [`value_used`] over the
//! real node chain from the root to the call: Prism always materializes a
//! `StatementsNode` where whitequark elides a single-statement `begin`, and
//! models `else` branches as `ElseNode`, so those are treated as transparent
//! wrappers that hand the question to their parent.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `=~` in places where the `MatchData` returned by `#match` will not be used.";

const HIGHER_PRECEDENCE_OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"+", b"-", b"*", b"/", b"%", b"**", b">", b">=", b"<", b"<=", b"<<", b">>",
];

/// Use `=~` instead of `String#match` or `Regexp#match` in a context where the returned `MatchData` is not needed.
#[derive(Debug, Clone)]
pub struct RedundantMatch;

impl Rule for RedundantMatch {
    const META: RuleMeta = RuleMeta {
        name: "Performance/RedundantMatch",
        department: Department::Performance,
        summary: "Use `=~` instead of `String#match` or `Regexp#match` in a context where the returned `MatchData` is not needed.",
        explanation: "\
Identifies the use of `Regexp#match` or `String#match`, which returns
`#<MatchData>`/`nil`. The return value of `=~` is an integral index/`nil`
and is more performant.

```ruby
# bad
do_something if str.match(/regex/)
while regex.match('str')
  do_something
end

# good
method(str =~ /regex/)
return value unless regex =~ 'str'
```",
        enabled_by_default: true,
        severity: Severity::Convention,
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
        let Some(call) = node.as_call_node() else { return };
        if !match_call(&call) {
            return;
        }
        // `node.parent.block_type?` for the call that owns a block.
        if call.block().is_some_and(|b| b.as_block_node().is_some()) {
            return;
        }
        let chain = ancestor_chain(ctx);
        let own = node.span();
        if value_used(&chain, own) && !only_truthiness_matters(&chain, own) {
            return;
        }

        let (Some(receiver), Some(arg)) = (call.receiver(), first_argument(&call)) else { return };
        if !(is_regexp(&receiver) || is_regexp(&arg)) {
            ctx.report(&Self::META, own, MSG);
            return;
        }
        let mut new_source = ctx.text(receiver.span()).to_vec();
        new_source.extend_from_slice(b" =~ ");
        if requires_parentheses(ctx, &arg) {
            new_source.push(b'(');
            new_source.extend_from_slice(ctx.text(arg.span()));
            new_source.push(b')');
        } else {
            new_source.extend_from_slice(ctx.text(arg.span()));
        }
        ctx.report_with_fix(
            &Self::META,
            own,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::replace(own, new_source)] },
        );
    }
}

fn is_regexp(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::RegularExpressionNode | NodeKind::InterpolatedRegularExpressionNode
    )
}

fn is_str_or_regexp(node: &Node<'_>) -> bool {
    node.kind() == NodeKind::StringNode || is_regexp(node)
}

/// Whitequark counts a `&blk` argument among `arguments`.
fn first_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    call.arguments()
        .and_then(|a| a.arguments().iter().next())
        .or_else(|| call.block().filter(|b| b.as_block_argument_node().is_some()))
}

fn argument_count(call: &CallNode<'_>) -> usize {
    call.arguments().map_or(0, |a| a.arguments().iter().count())
        + usize::from(call.block().is_some_and(|b| b.as_block_argument_node().is_some()))
}

/// `{(send {str regexp} :match _) (send !nil? :match {str regexp})}`
fn match_call(call: &CallNode<'_>) -> bool {
    if call.is_safe_navigation() || call.name().as_slice() != b"match" || argument_count(call) != 1
    {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    if is_str_or_regexp(&receiver) {
        return true;
    }
    first_argument(call).is_some_and(|arg| is_str_or_regexp(&arg))
}

fn requires_parentheses(ctx: &Context<'_>, arg: &Node<'_>) -> bool {
    match arg.kind() {
        NodeKind::IfNode => {
            if arg.as_if_node().is_some_and(|n| n.if_keyword_loc().is_none()) {
                return true;
            }
            false
        }
        NodeKind::AndNode | NodeKind::OrNode | NodeKind::RangeNode => true,
        NodeKind::CallNode => {
            let call = arg.as_call_node().expect("kind matched");
            if call.block().is_some_and(|b| b.as_block_node().is_some()) {
                return false;
            }
            let parenthesized = call.closing_loc().is_some_and(|l| ctx.text(l.span()) == b")");
            requires_parentheses_for_call_like(
                parenthesized,
                argument_count(&call) > 0,
                call.name().as_slice(),
            )
        }
        NodeKind::YieldNode => {
            let node = arg.as_yield_node().expect("kind matched");
            requires_parentheses_for_call_like(
                node.rparen_loc().is_some(),
                node.arguments().is_some(),
                b"yield",
            )
        }
        NodeKind::SuperNode => {
            let node = arg.as_super_node().expect("kind matched");
            let has_args = node.arguments().is_some()
                || node.block().is_some_and(|b| b.as_block_argument_node().is_some());
            if node.block().is_some_and(|b| b.as_block_node().is_some()) {
                return false;
            }
            requires_parentheses_for_call_like(node.rparen_loc().is_some(), has_args, b"super")
        }
        _ => false,
    }
}

fn requires_parentheses_for_call_like(
    parenthesized: bool,
    has_arguments: bool,
    name: &[u8],
) -> bool {
    if parenthesized || !has_arguments {
        return false;
    }
    !HIGHER_PRECEDENCE_OPERATOR_METHODS.contains(&name)
}

/// The real nodes from the root down to (and including) the current node's
/// parent, rebuilt from `ctx.ancestors()`.
fn ancestor_chain<'a>(ctx: &'a Context<'_>) -> Vec<Node<'a>> {
    let mut chain: Vec<Node<'a>> = Vec::new();
    let mut current = ctx.parsed().root();
    for (i, info) in ctx.ancestors().iter().enumerate() {
        if i > 0 {
            let mut found = None;
            for_each_child(&current, |child| {
                if found.is_none() && child.kind() == info.kind && child.span() == info.span {
                    found = Some(*child);
                }
            });
            let Some(next) = found else { break };
            current = next;
        }
        chain.push(current);
    }
    chain
}

fn span_of_predicate(node: &Node<'_>) -> Option<Span> {
    match node.kind() {
        NodeKind::IfNode => node.as_if_node().map(|n| n.predicate().span()),
        NodeKind::UnlessNode => node.as_unless_node().map(|n| n.predicate().span()),
        NodeKind::WhileNode => node.as_while_node().map(|n| n.predicate().span()),
        NodeKind::UntilNode => node.as_until_node().map(|n| n.predicate().span()),
        NodeKind::CaseNode => node.as_case_node().and_then(|n| n.predicate()).map(|p| p.span()),
        _ => None,
    }
}

/// `^({if while until case while_post until_post} equal?(%0) ...)`
fn only_truthiness_matters(chain: &[Node<'_>], own: Span) -> bool {
    chain.last().and_then(span_of_predicate) == Some(own)
}

/// rubocop-ast's `Node#value_used?`.
fn value_used(chain: &[Node<'_>], child: Span) -> bool {
    let Some((parent, rest)) = chain.split_last() else { return false };
    let pspan = parent.span();
    match parent.kind() {
        NodeKind::ProgramNode => false,
        NodeKind::StatementsNode => child.end == pspan.end && value_used(rest, pspan),
        NodeKind::ParenthesesNode
        | NodeKind::EmbeddedStatementsNode
        | NodeKind::ArrayNode
        | NodeKind::DefinedNode
        | NodeKind::InterpolatedStringNode
        | NodeKind::InterpolatedSymbolNode
        | NodeKind::InterpolatedXStringNode
        | NodeKind::InterpolatedRegularExpressionNode
        | NodeKind::RangeNode
        | NodeKind::FlipFlopNode
        | NodeKind::HashNode
        | NodeKind::KeywordHashNode
        | NodeKind::AssocNode
        | NodeKind::WhenNode
        | NodeKind::ElseNode => value_used(rest, pspan),
        NodeKind::BeginNode => {
            let begin = parent.as_begin_node().expect("kind matched");
            if begin.rescue_clause().is_some()
                || begin.else_clause().is_some()
                || begin.ensure_clause().is_some()
            {
                true
            } else {
                value_used(rest, pspan)
            }
        }
        NodeKind::ForNode => {
            let body =
                parent.as_for_node().and_then(|n| n.statements()).map(|s| s.location().span());
            if body == Some(child) {
                value_used(rest, pspan)
            } else {
                true
            }
        }
        NodeKind::IfNode | NodeKind::UnlessNode | NodeKind::CaseNode => {
            span_of_predicate(parent) == Some(child) || value_used(rest, pspan)
        }
        NodeKind::WhileNode | NodeKind::UntilNode => span_of_predicate(parent) == Some(child),
        _ => true,
    }
}
