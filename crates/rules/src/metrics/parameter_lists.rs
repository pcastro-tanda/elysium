//! `Metrics/ParameterLists`, ported from RuboCop's
//! `lib/rubocop/cop/metrics/parameter_lists.rb`.
//!
//! Upstream's `exclude_limit 'Max'`/`exclude_limit 'MaxOptionalParameters'`
//! only ever writes to a `--auto-gen-config` tmp directory (see
//! `RuboCop::ExcludeLimit#exclude_limit`); `self.max = count` inside
//! `add_offense`'s block is therefore inert for an ordinary lint run and is
//! not reproduced here.
//!
//! whitequark's `args` node -- reached for a `def`'s parameter list, a
//! block's `|...|` list, and a stabby lambda's `(...)` list alike -- is
//! split across three distinct Prism shapes: [`NodeKind::DefNode`]'s own
//! `parameters` field (a bare [`ParametersNode`]), [`NodeKind::BlockNode`]'s
//! `parameters` field (a [`BlockParametersNode`] wrapping an inner
//! `ParametersNode`, or `None` for bare `do...end`), and
//! [`NodeKind::LambdaNode`]'s own `parameters` field (either shape,
//! depending on whether `->(...)` used parens). This port therefore
//! processes parameter lists directly off their three owning node kinds
//! (via a manual recursive walk mirroring `metrics/block_nesting.rs`,
//! needed for typed access to a `def`'s enclosing block/call) rather than
//! subscribing to a parameter-list node kind itself.
//!
//! `argument_to_lambda_or_proc?`'s `^lambda_or_proc?` (match on the args
//! node's parent) collapses to: a stabby lambda's own parameter list is
//! always exempt (its every route reaches a lambda literal), and a block's
//! parameter list is exempt when the block's owning call is
//! `lambda`/`proc`/`Proc.new` ([`ruby_ast::ext::is_lambda_or_proc`]).
//!
//! `struct_new_or_data_define_block?`'s `(block (send ...) (args) ...)`
//! pattern requires the def's whitequark parent to literally be the
//! `Struct.new`/`Data.define` block node, which only holds when `initialize`
//! is the block's *sole* statement -- multiple statements make whitequark's
//! parent a `begin` node that fails to match `(block ...)`. Prism always
//! wraps a body in a `StatementsNode` (even a single-statement one), so this
//! port checks that the `DefNode`'s enclosing `StatementsNode` holds exactly
//! one statement before checking that its own parent is such a block.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const, is_lambda_or_proc};
use ruby_ast::node::{BlockNode, CallNode, DefNode, ParametersNode};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Avoid parameter lists longer than three or four parameters.
#[derive(Debug, Clone)]
pub struct ParameterLists {
    max: i64,
    count_keyword_args: bool,
    max_optional_parameters: i64,
}

impl Rule for ParameterLists {
    const META: RuleMeta = RuleMeta {
        name: "Metrics/ParameterLists",
        department: Department::Metrics,
        summary: "Avoid parameter lists longer than three or four parameters.",
        explanation: "Checks for methods with too many parameters.\n\n\
            The maximum number of parameters is configurable. Keyword arguments can \
            optionally be excluded from the total count, as they add less complexity than \
            positional or optional parameters.\n\n\
            Any number of arguments for `initialize` inside a block of `Struct.new` or \
            `Data.define` is always allowed, since checking the number of arguments of that \
            `initialize` method does not make sense.\n\n\
            NOTE: An explicit block argument (`&block`) is never counted, to prevent an \
            erroneous change that is avoided by making the block argument implicit.\n\n\
            This cop also checks for the maximum number of optional parameters, configurable \
            via `MaxOptionalParameters`.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[],
        config: &[
            ConfigOption {
                name: "Max",
                default: ConfigDefault::Int(5),
                allowed: &[],
                doc: "Maximum number of parameters allowed.",
            },
            ConfigOption {
                name: "CountKeywordArgs",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Count keyword args towards the maximum.",
            },
            ConfigOption {
                name: "MaxOptionalParameters",
                default: ConfigDefault::Int(3),
                allowed: &[],
                doc: "Maximum number of optional parameters allowed.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            max: options.int("Max"),
            count_keyword_args: options.bool("CountKeywordArgs"),
            max_optional_parameters: options.int("MaxOptionalParameters"),
        })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let mut ancestors = Vec::new();
        walk(&root, &mut ancestors, ctx, self);
    }
}

/// Upstream's `MSG`.
fn message(max: i64, count: u32) -> String {
    format!("Avoid parameter lists longer than {max} parameters. [{count}/{max}]")
}

/// Upstream's `OPTIONAL_PARAMETERS_MSG`.
fn optional_parameters_message(max: i64, count: u32) -> String {
    format!("Method has too many optional parameters. [{count}/{max}]")
}

fn walk<'pr>(
    node: &Node<'pr>,
    ancestors: &mut Vec<Node<'pr>>,
    ctx: &mut Context<'_>,
    rule: &ParameterLists,
) {
    match node.kind() {
        NodeKind::DefNode => {
            let def = node.as_def_node().expect("kind matched");
            check_optional_parameters(&def, ctx, rule.max_optional_parameters);
            check_def_parameter_count(&def, ancestors, ctx, rule);
        }
        NodeKind::BlockNode => {
            let block = node.as_block_node().expect("kind matched");
            check_block_parameter_count(&block, ancestors, ctx, rule);
        }
        // A stabby lambda's parameter list is always exempt
        // (`argument_to_lambda_or_proc?` matches every route to it), same as
        // any other node kind this walk does not otherwise recognize.
        _ => {}
    }
    ancestors.push(*node);
    for_each_child(node, |child| walk(child, ancestors, ctx, rule));
    ancestors.pop();
}

/// Upstream's `on_def`/`on_defs` (aliased): flags `optarg_type?` parameters
/// (`a = 1`), excluding optional keyword parameters (`a: 1`).
fn check_optional_parameters(
    def: &DefNode<'_>,
    ctx: &mut Context<'_>,
    max_optional_parameters: i64,
) {
    let Some(params) = def.parameters() else { return };
    let count = u32::try_from(params.optionals().len()).unwrap_or(u32::MAX);
    if i64::from(count) <= max_optional_parameters {
        return;
    }
    ctx.report(
        &ParameterLists::META,
        def.as_node().span(),
        optional_parameters_message(max_optional_parameters, count),
    );
}

/// Upstream's `on_args`, for a `def`'s own parameter list.
fn check_def_parameter_count<'pr>(
    def: &DefNode<'pr>,
    ancestors: &[Node<'pr>],
    ctx: &mut Context<'_>,
    rule: &ParameterLists,
) {
    let Some(params) = def.parameters() else { return };
    if is_struct_or_data_initialize(def, ancestors) {
        return;
    }
    let count = args_count(&params, rule.count_keyword_args);
    if i64::from(count) <= rule.max {
        return;
    }
    let span = match (def.lparen_loc(), def.rparen_loc()) {
        (Some(lparen), Some(rparen)) => Span::new(lparen.span().start, rparen.span().end),
        _ => params.as_node().span(),
    };
    ctx.report(&ParameterLists::META, span, message(rule.max, count));
}

/// Upstream's `on_args`, for a block's `|...|` parameter list. `ancestors`'
/// last entry is the block's own parent, i.e. the owning `CallNode`.
fn check_block_parameter_count<'pr>(
    block: &BlockNode<'pr>,
    ancestors: &[Node<'pr>],
    ctx: &mut Context<'_>,
    rule: &ParameterLists,
) {
    let Some(params_field) = block.parameters() else { return };
    let Some(block_params) = params_field.as_block_parameters_node() else {
        // `NumberedParametersNode`/`ItParametersNode`: no explicit `args`
        // node upstream, so `on_args` never fires for these.
        return;
    };
    let Some(params) = block_params.parameters() else { return };
    if ancestors.last().and_then(Node::as_call_node).is_some_and(|call| is_lambda_or_proc(&call)) {
        return;
    }
    let count = args_count(&params, rule.count_keyword_args);
    if i64::from(count) <= rule.max {
        return;
    }
    ctx.report(&ParameterLists::META, block_params.as_node().span(), message(rule.max, count));
}

/// Upstream's `args_count`: every declared parameter except an explicit
/// block argument (`&block`, always excluded); a named keyword parameter
/// (`kwarg`/`kwoptarg` -- Prism's `RequiredKeywordParameterNode`/
/// `OptionalKeywordParameterNode`, held in `keywords`) additionally excluded
/// when `CountKeywordArgs` is `false`. A keyword-rest parameter (`**kwrest`,
/// including anonymous `**`/`**nil`) is not a `NAMED_KEYWORD_TYPES` member
/// upstream and so always counts, regardless of `CountKeywordArgs`.
fn args_count(params: &ParametersNode<'_>, count_keyword_args: bool) -> u32 {
    let mut count = u32::try_from(params.requireds().len()).unwrap_or(u32::MAX)
        + u32::try_from(params.optionals().len()).unwrap_or(u32::MAX)
        + u32::from(params.rest().is_some())
        + u32::try_from(params.posts().len()).unwrap_or(u32::MAX)
        + u32::from(params.keyword_rest().is_some());
    if count_keyword_args {
        count += u32::try_from(params.keywords().len()).unwrap_or(u32::MAX);
    }
    count
}

/// Upstream's `parent.method?(:initialize) &&
/// struct_new_or_data_define_block?(parent.parent)`.
fn is_struct_or_data_initialize<'pr>(def: &DefNode<'pr>, ancestors: &[Node<'pr>]) -> bool {
    if def.name().as_slice() != b"initialize" {
        return false;
    }
    let [.., call, block, statements] = ancestors else { return false };
    let Some(statements) = statements.as_statements_node() else { return false };
    if statements.body().len() != 1 {
        return false;
    }
    let Some(block) = block.as_block_node() else { return false };
    if !block_has_no_explicit_params(&block) {
        return false;
    }
    let Some(call) = call.as_call_node() else { return false };
    is_struct_new_or_data_define(&call)
}

/// Upstream's `(args)` (a block parameter list with zero children): no
/// explicit parameter list at all, or one with every field empty.
fn block_has_no_explicit_params(block: &BlockNode<'_>) -> bool {
    let Some(params_field) = block.parameters() else { return true };
    let Some(block_params) = params_field.as_block_parameters_node() else { return false };
    match block_params.parameters() {
        None => true,
        Some(params) => {
            params.requireds().is_empty()
                && params.optionals().is_empty()
                && params.rest().is_none()
                && params.posts().is_empty()
                && params.keywords().is_empty()
                && params.keyword_rest().is_none()
                && params.block().is_none()
        }
    }
}

/// Upstream's `(send (const {nil? cbase} :Struct) :new ...)` /
/// `(send (const {nil? cbase} :Data) :define ...)`.
fn is_struct_new_or_data_define(call: &CallNode<'_>) -> bool {
    let Some(receiver) = call.receiver() else { return false };
    if !is_bare_or_toplevel_const(&receiver) {
        return false;
    }
    let Some(name) = const_name(&receiver) else { return false };
    let method = call.name();
    let method = method.as_slice();
    (name == "Struct" && method == b"new") || (name == "Data" && method == b"define")
}
