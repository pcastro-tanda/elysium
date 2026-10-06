//! `Performance/MapMethodChain`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/map_method_chain.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Checks if the map method is used in a chain.
#[derive(Debug, Clone)]
pub struct MapMethodChain {
    ignored: Vec<Span>,
}

fn is_map(call: &CallNode<'_>) -> bool {
    matches!(call.name().as_slice(), b"map" | b"collect")
}

/// `block_pass_with_symbol_arg?(node.first_argument)`: the call's first
/// whitequark argument is a `&:sym` block pass (so no positional arguments).
fn block_pass_symbol(call: &CallNode<'_>) -> Option<String> {
    if call.arguments().is_some() {
        return None;
    }
    let block = call.block()?;
    let expr = block.as_block_argument_node()?.expression()?;
    let sym = expr.as_symbol_node()?;
    Some(String::from_utf8_lossy(sym.unescaped()).into_owned())
}

/// `find_begin_of_chained_map_method`.
fn find_begin<'pr>(call: &CallNode<'pr>, map_args: &mut Vec<String>) -> Option<CallNode<'pr>> {
    let mut current = call.receiver()?.as_call_node()?;
    loop {
        if !is_map(&current) {
            return None;
        }
        map_args.insert(0, block_pass_symbol(&current)?);
        let next = current.receiver().and_then(|r| r.as_call_node());
        match next {
            Some(receiver) if block_pass_symbol(&receiver).is_some() => current = receiver,
            _ => return Some(current),
        }
    }
}

impl Rule for MapMethodChain {
    const META: RuleMeta = RuleMeta {
        name: "Performance/MapMethodChain",
        department: Department::Performance,
        summary: "Checks if the `map` method is used in a chain.",
        explanation: "Checks if the map method is used in a chain.\n\nAutocorrection is not supported because an appropriate block variable name cannot be determined automatically.\n\nThis cop is unsafe because false positives occur if the number of times the first method is executed affects the return value of subsequent methods.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { ignored: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() || !is_map(&call) {
            return;
        }
        let whole = node.span();
        if self.ignored.iter().any(|s| s.start <= whole.start && whole.end <= s.end) {
            return;
        }
        let Some(map_arg) = block_pass_symbol(&call) else { return };
        let mut map_args = vec![map_arg];
        let Some(begin) = find_begin(&call, &mut map_args) else { return };
        let Some(selector) = begin.message_loc() else { return };
        let end = call_span_excluding_block(&call).end;
        let range = Span::new(selector.span().start, end);
        let message = format!(
            "Use `{name} {{ |x| x.{args} }}` instead of `{name}` method chain.",
            name = String::from_utf8_lossy(begin.name().as_slice()),
            args = map_args.join(".")
        );
        ctx.report(&Self::META, range, message);
        self.ignored.push(whole);
    }
}
