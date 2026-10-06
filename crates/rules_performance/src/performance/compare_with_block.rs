//! `Performance/CompareWithBlock`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/compare_with_block.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use `sort_by(&:foo)` instead of `sort { |a, b| a.foo <=> b.foo }`.
#[derive(Debug, Clone)]
pub struct CompareWithBlock;

impl Rule for CompareWithBlock {
    const META: RuleMeta = RuleMeta {
        name: "Performance/CompareWithBlock",
        department: Department::Performance,
        summary: "Use `sort_by(&:foo)` instead of `sort { |a, b| a.foo <=> b.foo }`.",
        explanation: "Identifies places where `sort { |a, b| a.foo <=> b.foo }` can be replaced \
                      by `sort_by(&:foo)`. This cop also checks `sort!`, `min`, `max` and \
                      `minmax` methods.\n\n```ruby\n# bad\narray.sort { |a, b| a.foo <=> b.foo }\n\
                      array.sort { |a, b| a[:foo] <=> b[:foo] }\n\n# good\n\
                      array.sort_by(&:foo)\narray.sort_by { |a| a[:foo] }\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(send) = node.as_call_node() else { return };
        if send.is_safe_navigation() || send.arguments().is_some() {
            return;
        }
        let replacement_method = match send.name().as_slice() {
            b"sort" => "sort_by",
            b"sort!" => "sort_by!",
            b"min" => "min_by",
            b"max" => "max_by",
            b"minmax" => "minmax_by",
            _ => return,
        };
        let Some(block_node) = send.block() else { return };
        let Some(block) = block_node.as_block_node() else { return };

        // `(args (arg $_a) (arg $_b))`
        let Some(params) = block.parameters() else { return };
        let Some(block_params) = params.as_block_parameters_node() else { return };
        if block_params.locals().iter().next().is_some() {
            return;
        }
        let Some(parameters) = block_params.parameters() else { return };
        if parameters.optionals().iter().next().is_some()
            || parameters.posts().iter().next().is_some()
            || parameters.keywords().iter().next().is_some()
            || parameters.rest().is_some()
            || parameters.keyword_rest().is_some()
            || parameters.block().is_some()
        {
            return;
        }
        let requireds: Vec<Node<'_>> = parameters.requireds().iter().collect();
        let [first, second] = requireds.as_slice() else { return };
        let (Some(first), Some(second)) =
            (first.as_required_parameter_node(), second.as_required_parameter_node())
        else {
            return;
        };
        let var_a = first.name();
        let var_b = second.name();
        let (var_a, var_b) = (var_a.as_slice(), var_b.as_slice());

        // `$send`: the whole body is a single call.
        let Some(body) = block.body() else { return };
        let Some(statements) = body.as_statements_node() else { return };
        let stmts: Vec<Node<'_>> = statements.body().iter().collect();
        let [body] = stmts.as_slice() else { return };
        let Some(compare) = body.as_call_node() else { return };

        // `(send (send (lvar a) $method $...) :<=> (send (lvar b) _method $...))`
        if compare.is_safe_navigation() || compare.name().as_slice() != b"<=>" {
            return;
        }
        if compare.block().is_some() {
            return;
        }
        let Some(left) = compare.receiver() else { return };
        let Some(left) = left.as_call_node() else { return };
        let Some(compare_args) = compare.arguments() else { return };
        let compare_args: Vec<Node<'_>> = compare_args.arguments().iter().collect();
        let [right] = compare_args.as_slice() else { return };
        let Some(right) = right.as_call_node() else { return };
        if left.is_safe_navigation()
            || right.is_safe_navigation()
            || left.block().is_some()
            || right.block().is_some()
        {
            return;
        }
        if !is_lvar(left.receiver(), var_a) || !is_lvar(right.receiver(), var_b) {
            return;
        }
        if left.name().as_slice() != right.name().as_slice() {
            return;
        }
        let method = left.name();
        let method = method.as_slice();
        let args_a: Vec<Node<'_>> =
            left.arguments().map_or_else(Vec::new, |a| a.arguments().iter().collect());
        let args_b: Vec<Node<'_>> =
            right.arguments().map_or_else(Vec::new, |a| a.arguments().iter().collect());

        // `slow_compare?`
        if args_a.len() != args_b.len() {
            return;
        }
        let is_index = method == b"[]";
        if is_index {
            let [a] = args_a.as_slice() else { return };
            let [b] = args_b.as_slice() else { return };
            if !literal_key_eq(a, b, ctx) {
                return;
            }
        } else if !args_a.is_empty() {
            return;
        }

        let Some(selector) = send.message_loc() else { return };
        let range = Span::new(selector.span().start, block.closing_loc().span().end);
        let compare_method = String::from_utf8_lossy(send.name().as_slice()).into_owned();
        let var_a = String::from_utf8_lossy(var_a).into_owned();
        let var_b = String::from_utf8_lossy(var_b).into_owned();
        let (instead, str_a, str_b, replacement) = if is_index {
            let key = String::from_utf8_lossy(ctx.text(args_a[0].span())).into_owned();
            let instead = format!(" {{ |a| a[{key}] }}");
            (instead.clone(), format!("{var_a}[{key}]"), format!("{var_b}[{key}]"), {
                format!("{replacement_method}{instead}")
            })
        } else {
            let method = String::from_utf8_lossy(method).into_owned();
            let instead = format!("(&:{method})");
            (
                instead.clone(),
                format!("{var_a}.{method}"),
                format!("{var_b}.{method}"),
                format!("{replacement_method}{instead}"),
            )
        };
        let message = format!(
            "Use `{replacement_method}{instead}` instead of `{compare_method} {{ |{var_a}, \
             {var_b}| {str_a} <=> {str_b} }}`."
        );
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, replacement.into_bytes())],
            },
        );
    }
}

/// `(lvar %name)`.
fn is_lvar(node: Option<Node<'_>>, name: &[u8]) -> bool {
    node.and_then(|n| n.as_local_variable_read_node().map(|l| l.name().as_slice() == name))
        .unwrap_or(false)
}

/// `args_a == args_b` restricted to the `sym`/`str`/`int` keys that
/// `slow_compare?` accepts.
fn literal_key_eq(a: &Node<'_>, b: &Node<'_>, ctx: &Context<'_>) -> bool {
    if let (Some(a), Some(b)) = (a.as_symbol_node(), b.as_symbol_node()) {
        return a.unescaped() == b.unescaped();
    }
    if let (Some(a), Some(b)) = (a.as_string_node(), b.as_string_node()) {
        return a.unescaped() == b.unescaped();
    }
    if a.as_integer_node().is_some() && b.as_integer_node().is_some() {
        return ctx.text(a.span()) == ctx.text(b.span());
    }
    false
}
