//! `Performance/SortReverse`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/sort_reverse.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::BlockNode;
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

/// Use `sort.reverse` instead of `sort { |a, b| b <=> a }`.
#[derive(Debug, Clone)]
pub struct SortReverse;

impl Rule for SortReverse {
    const META: RuleMeta = RuleMeta {
        name: "Performance/SortReverse",
        department: Department::Performance,
        summary: "Use `sort.reverse` instead of `sort { |a, b| b <=> a }`.",
        explanation: "\
Identifies places where `sort { |a, b| b <=> a }` can be replaced by a
faster `sort.reverse`.

```ruby
# bad
array.sort { |a, b| b <=> a }

# good
array.sort.reverse
```",
        enabled_by_default: false,
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
        if call.name().as_slice() != b"sort" || call.arguments().is_some() {
            return;
        }
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
        let Some((var_a, var_b)) = block_variables(&block) else { return };
        if !replaceable_body(block.body(), &var_b, &var_a) {
            return;
        }
        // `send.loc.dot` is nil for a receiverless `sort`.
        let Some(dot) = call.call_operator_loc() else { return };
        let Some(selector) = call.message_loc() else { return };
        let range = Span::new(selector.span().start, block.closing_loc().span().end);
        let prefer = format!("sort{}reverse", String::from_utf8_lossy(ctx.text(dot.span())));
        let message = format!("Use `{prefer}` instead.");
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, prefer.into_bytes())],
            },
        );
    }
}

/// `(args (arg a) (arg b))` or a numblock using `_1`/`_2` (`arg_count == 2`).
fn block_variables(block: &BlockNode<'_>) -> Option<(Vec<u8>, Vec<u8>)> {
    let params = block.parameters()?;
    if let Some(numbered) = params.as_numbered_parameters_node() {
        return (numbered.maximum() == 2).then(|| (b"_1".to_vec(), b"_2".to_vec()));
    }
    let params = params.as_block_parameters_node()?;
    if params.locals().iter().next().is_some() {
        return None;
    }
    let list = params.parameters()?;
    if list.optionals().iter().next().is_some()
        || list.rest().is_some()
        || list.posts().iter().next().is_some()
        || list.keywords().iter().next().is_some()
        || list.keyword_rest().is_some()
        || list.block().is_some()
    {
        return None;
    }
    let mut requireds = list.requireds().iter();
    let (Some(a), Some(b), None) = (requireds.next(), requireds.next(), requireds.next()) else {
        return None;
    };
    let a = a.as_required_parameter_node()?;
    let b = b.as_required_parameter_node()?;
    Some((a.name().as_slice().to_vec(), b.name().as_slice().to_vec()))
}

/// `(send (lvar receiver_name) :<=> (lvar argument_name))` as the sole body.
fn replaceable_body(body: Option<Node<'_>>, receiver_name: &[u8], argument_name: &[u8]) -> bool {
    let Some(stmts) = body.as_ref().and_then(Node::as_statements_node) else { return false };
    let mut it = stmts.body().iter();
    let (Some(only), None) = (it.next(), it.next()) else { return false };
    let Some(send) = only.as_call_node() else { return false };
    if send.is_safe_navigation() || send.name().as_slice() != b"<=>" || send.block().is_some() {
        return false;
    }
    let Some(recv) = send.receiver() else { return false };
    let Some(args) = send.arguments() else { return false };
    let mut args = args.arguments().iter();
    let (Some(arg), None) = (args.next(), args.next()) else { return false };
    let is_lvar = |n: &Node<'_>, name: &[u8]| {
        n.as_local_variable_read_node().is_some_and(|l| l.name().as_slice() == name)
    };
    is_lvar(&recv, receiver_name) && is_lvar(&arg, argument_name)
}
