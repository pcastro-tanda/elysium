//! `Performance/Sum`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/sum.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use `sum` instead of a custom array summation.
#[derive(Debug, Clone)]
pub struct Sum {
    /// `minimum_target_ruby_version 2.4`.
    supported: bool,
    only_sum_or_with_initial_value: bool,
    /// `safe_autocorrect?` (`Safe` is never `false` for this cop).
    safe_autocorrect: bool,
}

impl Rule for Sum {
    const META: RuleMeta = RuleMeta {
        name: "Performance/Sum",
        department: Department::Performance,
        summary: "Use `sum` instead of a custom array summation.",
        explanation: "Identifies places where custom code finding the sum of elements in some \
                      Enumerable object can be replaced by `Enumerable#sum` method.\n\n\
                      Autocorrections are unproblematic wherever an initial value is provided \
                      explicitly. When no initial value is provided, `Enumerable#reduce` picks \
                      the first enumerated value as initial value whereas `Enumerable#sum` sets \
                      an initial value of `0`, which can lead to a `TypeError`; those \
                      autocorrections are unsafe.\n\n```ruby\n# bad\n[1, 2, 3].inject(:+)\n\
                      [1, 2, 3].inject(&:+)\n[1, 2, 3].reduce { |acc, elem| acc + elem }\n\
                      [1, 2, 3].reduce(10, :+)\n[1, 2, 3].map { |elem| elem ** 2 }.sum\n\
                      [1, 2, 3].collect(&:count).sum(10)\n\n# good\n[1, 2, 3].sum\n\
                      [1, 2, 3].sum(10)\n[1, 2, 3].sum { |elem| elem ** 2 }\n\
                      [1, 2, 3].sum(10, &:count)\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "SafeAutoCorrect",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Do not autocorrect when no initial value is given.",
            },
            ConfigOption {
                name: "OnlySumOrWithInitialValue",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Only flag `sum` candidates that carry an initial value.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            supported: options.target_ruby_version() >= 2.4,
            only_sum_or_with_initial_value: options.bool("OnlySumOrWithInitialValue"),
            safe_autocorrect: options.bool("SafeAutoCorrect"),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if matches!(call.name().as_slice(), b"inject" | b"reduce" | b"sum")
            && !is_empty_array_literal(&call)
        {
            self.handle_sum_candidate(&call, ctx);
            self.handle_sum_map_candidate(&call, ctx);
        }
        if call.block().is_some_and(|block| block.as_block_node().is_some()) {
            self.on_block(&call, ctx);
        }
    }
}

impl Sum {
    fn handle_sum_candidate(&self, node: &CallNode<'_>, ctx: &mut Context<'_>) {
        let method = match node.name().as_slice() {
            b"inject" => "inject",
            b"reduce" => "reduce",
            _ => return,
        };
        let args = argument_list(node);
        let (init, operation) = match args.as_slice() {
            [operation] => (None, operation),
            [init, operation] => (Some(init), operation),
            _ => return,
        };
        let block_pass = if let Some(block_pass) = operation.as_block_argument_node() {
            if !block_pass.expression().is_some_and(|expr| is_plus_symbol(&expr)) {
                return;
            }
            true
        } else if is_plus_symbol(operation) {
            false
        } else {
            return;
        };
        if self.only_sum_or_with_initial_value && init.is_none() {
            return;
        }

        let selector_start =
            node.message_loc().map_or(node.as_node().span().start, |l| l.span().start);
        let range = Span::new(selector_start, call_span_excluding_block(node).end);
        let good_method = build_good_method(init, None, ctx);
        let mut bad_method = format!("{method}(");
        if let Some(init) = init {
            bad_method.push_str(&source(ctx, init));
            bad_method.push_str(", ");
        }
        bad_method.push_str(if block_pass { "&:+)" } else { ":+)" });
        let array_literal =
            node.receiver().is_some_and(|receiver| receiver.as_array_node().is_some());
        let message = if init.is_none() && !array_literal {
            format!(
                "Use `{good_method}` instead of `{bad_method}`, unless calling `{bad_method}` \
                 on an empty array."
            )
        } else {
            format!("Use `{good_method}` instead of `{bad_method}`.")
        };
        self.report(ctx, range, message, init.is_none(), good_method);
    }

    fn handle_sum_map_candidate(&self, node: &CallNode<'_>, ctx: &mut Context<'_>) {
        if node.name().as_slice() != b"sum" {
            return;
        }
        let args: Vec<Node<'_>> =
            node.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
        let init = match args.as_slice() {
            [] => None,
            [init] => Some(init),
            _ => return,
        };
        let Some(receiver) = node.receiver().and_then(|r| r.as_call_node()) else { return };
        if !matches!(receiver.name().as_slice(), b"map" | b"collect") {
            return;
        }
        let map_block = receiver.block();
        let map_has_args = receiver.arguments().is_some();
        let block_pass = match &map_block {
            Some(block) if block.as_block_node().is_some() => {
                // whitequark's `(block ...)` excludes numblock/itblock.
                if map_has_args
                    || block.as_block_node().is_some_and(|b| {
                        b.parameters().is_some_and(|p| {
                            p.as_numbered_parameters_node().is_some()
                                || p.as_it_parameters_node().is_some()
                        })
                    })
                {
                    return;
                }
                None
            }
            Some(block) if block.as_block_argument_node().is_some() => {
                if map_has_args {
                    return;
                }
                Some(block.span())
            }
            _ => return,
        };
        // `node.block_literal? || node.block_argument?`
        if node.block().is_some() {
            return;
        }

        let dot_src = receiver.call_operator_loc().map(|l| source_span(ctx, l.span()));
        let sum_method = build_good_method(init, None, ctx);
        let good_method = format!("{sum_method} {{ ... }}");
        let bad_method = format!(
            "{} {{ ... }}{}{sum_method}",
            String::from_utf8_lossy(receiver.name().as_slice()),
            dot_src.as_deref().unwrap_or(".")
        );
        let message = format!("Use `{good_method}` instead of `{bad_method}`.");

        let map_selector =
            receiver.message_loc().map_or(receiver.as_node().span().start, |l| l.span().start);
        let range = Span::new(map_selector, node.as_node().span().end);

        let sum_range = method_call_with_args_range(node.receiver(), node.as_node().span());
        let map_span = call_span_excluding_block(&receiver);
        let map_range = method_call_with_args_range(receiver.receiver(), map_span);
        let pass_src = block_pass.map(|span| source_span(ctx, span));
        let replacement = build_good_method_src(init, pass_src.as_deref(), ctx);
        let dot = dot_src.unwrap_or_default();
        self.report_edits(
            ctx,
            range,
            message,
            vec![
                Edit::delete(sum_range),
                Edit::replace(map_range, format!("{dot}{replacement}").into_bytes()),
            ],
        );
    }

    fn on_block(&self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        if !matches!(call.name().as_slice(), b"inject" | b"reduce") {
            return;
        }
        let args: Vec<Node<'_>> =
            call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
        let init = match args.as_slice() {
            [] => None,
            [init] => Some(init),
            _ => return,
        };
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
        let Some(params) = block.parameters().and_then(|p| p.as_block_parameters_node()) else {
            return;
        };
        if params.locals().iter().next().is_some() {
            return;
        }
        let Some(params) = params.parameters() else { return };
        if params.optionals().iter().next().is_some()
            || params.rest().is_some()
            || params.posts().iter().next().is_some()
            || params.keywords().iter().next().is_some()
            || params.keyword_rest().is_some()
            || params.block().is_some()
        {
            return;
        }
        let requireds: Vec<Node<'_>> = params.requireds().iter().collect();
        let [acc, elem] = requireds.as_slice() else { return };
        let (Some(acc), Some(elem)) =
            (acc.as_required_parameter_node(), elem.as_required_parameter_node())
        else {
            return;
        };
        let acc = acc.name();
        let elem = elem.name();

        let Some(body) = block.body().and_then(|b| b.as_statements_node()) else { return };
        let mut statements = body.body().iter();
        let (Some(body), None) = (statements.next(), statements.next()) else { return };
        let Some(body_call) = body.as_call_node() else { return };
        if body_call.is_safe_navigation()
            || body_call.name().as_slice() != b"+"
            || body_call.block().is_some()
        {
            return;
        }
        let Some(lhs) = body_call.receiver().and_then(|r| lvar_name(&r)) else { return };
        let body_args: Vec<Node<'_>> =
            body_call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
        let [rhs] = body_args.as_slice() else { return };
        let Some(rhs) = lvar_name(rhs) else { return };
        let acc = acc.as_slice();
        let elem = elem.as_slice();
        if !((lhs == acc && rhs == elem) || (lhs == elem && rhs == acc)) {
            return;
        }

        let selector_start =
            call.message_loc().map_or(call.as_node().span().start, |l| l.span().start);
        let range = Span::new(selector_start, block.as_node().span().end);
        let good_method = build_good_method(init, None, ctx);
        let mut bad_method = String::from_utf8_lossy(call.name().as_slice()).into_owned();
        if let Some(init) = init {
            bad_method.push('(');
            bad_method.push_str(&source(ctx, init));
            bad_method.push(')');
        }
        bad_method = format!(
            "{bad_method} {{ |{}, {}| {} }}",
            String::from_utf8_lossy(acc),
            String::from_utf8_lossy(elem),
            source(ctx, &body)
        );
        let message = format!("Use `{good_method}` instead of `{bad_method}`.");
        self.report(ctx, range, message, init.is_none(), good_method);
    }

    fn report(
        &self,
        ctx: &mut Context<'_>,
        range: Span,
        message: String,
        no_init: bool,
        replacement: String,
    ) {
        let edits = if no_init && self.safe_autocorrect {
            Vec::new()
        } else {
            vec![Edit::replace(range, replacement.into_bytes())]
        };
        self.report_edits(ctx, range, message, edits);
    }

    fn report_edits(&self, ctx: &mut Context<'_>, range: Span, message: String, edits: Vec<Edit>) {
        if edits.is_empty() {
            ctx.report(&Self::META, range, message);
            return;
        }
        let applicability =
            if self.safe_autocorrect { Applicability::Safe } else { Applicability::Unsafe };
        ctx.report_with_fix(&Self::META, range, message, Fix { applicability, edits });
    }
}

/// `array_literal?(node) && receiver.children.empty?`.
fn is_empty_array_literal(call: &CallNode<'_>) -> bool {
    call.receiver()
        .and_then(|receiver| receiver.as_array_node())
        .is_some_and(|array| array.elements().iter().next().is_none())
}

/// `(sym :+)`.
fn is_plus_symbol(node: &Node<'_>) -> bool {
    node.as_symbol_node().is_some_and(|sym| sym.unescaped() == b"+")
}

fn lvar_name(node: &Node<'_>) -> Option<Vec<u8>> {
    node.as_local_variable_read_node().map(|read| read.name().as_slice().to_vec())
}

/// The call's arguments the way whitequark lists a `send`'s: a `&block`
/// argument is one of them.
fn argument_list<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut args: Vec<Node<'pr>> =
        call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
    if let Some(block) = call.block().filter(|block| block.as_block_argument_node().is_some()) {
        args.push(block);
    }
    args
}

fn source(ctx: &Context<'_>, node: &Node<'_>) -> String {
    source_span(ctx, node.span())
}

fn source_span(ctx: &Context<'_>, span: Span) -> String {
    String::from_utf8_lossy(ctx.text(span)).into_owned()
}

/// `init.int_type? && init.value.zero?`.
fn is_zero_int(node: &Node<'_>) -> bool {
    node.as_integer_node().is_some_and(|int| {
        let value: Result<i32, _> = int.value().try_into();
        value == Ok(0)
    })
}

fn build_good_method(
    init: Option<&Node<'_>>,
    block_pass: Option<&Node<'_>>,
    ctx: &Context<'_>,
) -> String {
    let pass = block_pass.map(|node| source(ctx, node));
    build_good_method_src(init, pass.as_deref(), ctx)
}

fn build_good_method_src(
    init: Option<&Node<'_>>,
    block_pass: Option<&str>,
    ctx: &Context<'_>,
) -> String {
    let mut args: Vec<String> = Vec::new();
    if let Some(init) = init {
        if !is_zero_int(init) {
            args.push(source(ctx, init));
        }
    }
    if let Some(block_pass) = block_pass {
        args.push(block_pass.to_string());
    }
    if args.is_empty() {
        "sum".to_string()
    } else {
        format!("sum({})", args.join(", "))
    }
}

/// `receiver.source_range.end.join(node.source_range.end)`, else the whole node.
fn method_call_with_args_range(receiver: Option<Node<'_>>, node: Span) -> Span {
    match receiver {
        Some(receiver) => Span::new(receiver.span().end, node.end),
        None => node,
    }
}
