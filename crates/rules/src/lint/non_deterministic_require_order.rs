//! `Lint/NonDeterministicRequireOrder`, ported from RuboCop's
//! `lib/rubocop/cop/lint/non_deterministic_require_order.rb`.
//!
//! Upstream's `maximum_target_ruby_version 2.7` disables the whole cop once
//! the target Ruby is 3.0 or newer (where `Dir.glob`/`Dir[]` sort their
//! results by default); mirrored here as a plain `target_ruby_version > 2.7`
//! guard, matching `lint/erb_new_arguments.rs`'s approach.
//!
//! Prism has no `on_block`/`on_numblock`/`on_block_pass` split: a call's
//! `do...end` block (`BlockNode`) and an explicit `&block` pass
//! (`BlockArgumentNode`) are both just `CallNode::block()`, so both upstream
//! hooks collapse into one `CallNode` visit that branches on the block
//! node's kind. Numbered (`_1`) and `it` implicit block parameters read back
//! as `LocalVariableReadNode`/`ItLocalVariableReadNode` respectively.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockArgumentNode, BlockNode, CallNode};
use ruby_ast::{each_descendant, ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Sort files before requiring them.";

/// Always sort arrays returned by Dir.glob when requiring files.
#[derive(Debug, Clone)]
pub struct NonDeterministicRequireOrder {
    target_ruby_version: f32,
}

impl Rule for NonDeterministicRequireOrder {
    const META: RuleMeta = RuleMeta {
        name: "Lint/NonDeterministicRequireOrder",
        department: Department::Lint,
        summary: "Always sort arrays returned by Dir.glob when requiring files.",
        explanation: "`Dir[...]` and `Dir.glob(...)` do not make any guarantees about \
            the order in which files are returned. The final order is determined by \
            the operating system and file system. This means that using them in cases \
            where the order matters, such as requiring files, can lead to intermittent \
            failures that are hard to debug. To ensure this doesn't happen, always sort \
            the list.\n\n\
            `Dir.glob` and `Dir[]` sort globbed results by default in Ruby 3.0. So all \
            bad cases are acceptable when Ruby 3.0 or higher are used.\n\n\
            NOTE: This cop will be deprecated and removed when supporting only Ruby 3.0 \
            and higher.\n\n\
            ```ruby\n\
            # bad\n\
            Dir[\"./lib/**/*.rb\"].each do |file|\n\
            \u{20}\u{20}require file\n\
            end\n\n\
            # good\n\
            Dir[\"./lib/**/*.rb\"].sort.each do |file|\n\
            \u{20}\u{20}require file\n\
            end\n\
            ```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        // Ruby 3.0+ sorts `Dir.glob`/`Dir[]` results itself.
        if self.target_ruby_version > 2.7 {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let Some(block) = call.block() else { return };
        match block.kind() {
            NodeKind::BlockNode => check_block(&call, &block, ctx),
            NodeKind::BlockArgumentNode => check_block_pass(&call, &block, ctx),
            _ => {}
        }
    }
}

/// Whether the (optional) receiver is the bare or top-level `Dir` constant.
fn is_dir_const(node: &Node<'_>) -> bool {
    ext::is_bare_or_toplevel_const(node) && ext::const_name(node).as_deref() == Some("Dir")
}

/// `(send (const {nil? cbase} :Dir) :glob ...)`.
fn unsorted_dir_block(call: &CallNode<'_>) -> bool {
    call.name().as_slice() == b"glob" && call.receiver().is_some_and(|r| is_dir_const(&r))
}

/// `(send (send (const {nil? cbase} :Dir) {:[] :glob} ...) :each)`.
fn unsorted_dir_each(call: &CallNode<'_>) -> bool {
    if call.name().as_slice() != b"each" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    let Some(recv_call) = receiver.as_call_node() else { return false };
    matches!(recv_call.name().as_slice(), b"[]" | b"glob")
        && recv_call.receiver().is_some_and(|r| is_dir_const(&r))
}

/// `unsorted_dir_block?(node) || unsorted_dir_each?(node)`, also reused for
/// `unsorted_dir_glob_pass?`/`unsorted_dir_each_pass?`: in Prism a `&block`
/// pass never appears inside `arguments()`, so the pass variants match the
/// exact same shape as their block counterparts.
fn unsorted_dir_loop(call: &CallNode<'_>) -> bool {
    unsorted_dir_block(call) || unsorted_dir_each(call)
}

/// `(block-pass (send nil? :method (sym {:require :require_relative})))`.
fn method_require(block_arg: &BlockArgumentNode<'_>) -> bool {
    let Some(expr) = block_arg.expression() else { return false };
    let Some(call) = expr.as_call_node() else { return false };
    if call.receiver().is_some() || call.name().as_slice() != b"method" {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let list = args.arguments();
    if list.len() != 1 {
        return false;
    }
    let Some(sym) = list.first().and_then(|n| n.as_symbol_node()) else { return false };
    matches!(sym.unescaped(), b"require" | b"require_relative")
}

/// The name a `var_is_required?` search should look for: a plain block
/// parameter's name, one of a numbered block's implicit `_1.._n`, or `it`.
enum LoopVar {
    Named(Vec<u8>),
    Numbered(u8),
    It,
}

/// `(args (arg $_))`: exactly one required positional parameter, nothing
/// else. Numbered (`_1`) and `it` implicit parameters have no
/// `BlockParametersNode` at all.
fn loop_variable(block: &BlockNode<'_>) -> Option<LoopVar> {
    let params = block.parameters()?;
    match params.kind() {
        NodeKind::BlockParametersNode => {
            let bp = params.as_block_parameters_node().expect("kind matched");
            let p = bp.parameters()?;
            if p.requireds().len() != 1
                || !p.optionals().is_empty()
                || p.rest().is_some()
                || !p.posts().is_empty()
                || !p.keywords().is_empty()
                || p.keyword_rest().is_some()
                || p.block().is_some()
            {
                return None;
            }
            let first = p.requireds().first().expect("len checked above");
            let req = first.as_required_parameter_node()?;
            Some(LoopVar::Named(req.name().as_slice().to_vec()))
        }
        NodeKind::NumberedParametersNode => {
            let np = params.as_numbered_parameters_node().expect("kind matched");
            Some(LoopVar::Numbered(np.maximum()))
        }
        NodeKind::ItParametersNode => Some(LoopVar::It),
        _ => None,
    }
}

/// `(send nil? {:require :require_relative} (lvar %1))`, searched
/// recursively through `body`.
fn var_is_required(body: &Node<'_>, var: &LoopVar) -> bool {
    let mut found = requires_var(body, var);
    if !found {
        each_descendant(body, &mut |n| {
            if !found && requires_var(n, var) {
                found = true;
            }
        });
    }
    found
}

fn requires_var(node: &Node<'_>, var: &LoopVar) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.receiver().is_some() {
        return false;
    }
    if !matches!(call.name().as_slice(), b"require" | b"require_relative") {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let list = args.arguments();
    if list.len() != 1 {
        return false;
    }
    let arg = list.first().expect("len checked above");
    match var {
        LoopVar::Named(name) => arg
            .as_local_variable_read_node()
            .is_some_and(|lv| lv.name().as_slice() == name.as_slice()),
        LoopVar::Numbered(max) => (1..=*max).any(|i| {
            let target = format!("_{i}").into_bytes();
            arg.as_local_variable_read_node()
                .is_some_and(|lv| lv.name().as_slice() == target.as_slice())
        }),
        LoopVar::It => matches!(arg.kind(), NodeKind::ItLocalVariableReadNode),
    }
}

/// `on_block`/`on_numblock`: `node.send_node` is the call excluding its
/// `do...end` block (`ext::call_span_excluding_block`); `correct_block`.
fn check_block(call: &CallNode<'_>, block: &Node<'_>, ctx: &mut Context<'_>) {
    if !unsorted_dir_loop(call) {
        return;
    }
    let block_node = block.as_block_node().expect("kind matched by caller");
    let Some(body) = block_node.body() else { return };
    let Some(var) = loop_variable(&block_node) else { return };
    if !var_is_required(&body, &var) {
        return;
    }
    let span = ext::call_span_excluding_block(call);
    let replacement = if unsorted_dir_block(call) {
        let mut text = ctx.text(span).to_vec();
        text.extend_from_slice(b".sort.each");
        text
    } else {
        let receiver = call.receiver().expect("unsorted_dir_each matched");
        let mut text = ctx.text(receiver.span()).to_vec();
        text.extend_from_slice(b".sort.each");
        text
    };
    ctx.report_with_fix(
        &NonDeterministicRequireOrder::META,
        span,
        MSG,
        Fix { applicability: Applicability::Unsafe, edits: vec![Edit::replace(span, replacement)] },
    );
}

/// `on_block_pass`: `node.parent` is `call` itself (Prism keeps a `&block`
/// pass out of `arguments()`, exposed only via `CallNode::block()`), so
/// `parent_node.last_argument&.block_pass_type?` is always true here and
/// `correct_block_pass` is the only branch ever taken.
fn check_block_pass(call: &CallNode<'_>, block: &Node<'_>, ctx: &mut Context<'_>) {
    let block_arg = block.as_block_argument_node().expect("kind matched by caller");
    if !method_require(&block_arg) || !unsorted_dir_loop(call) {
        return;
    }
    let span = call.as_node().span();
    if unsorted_dir_block(call) {
        // `Dir.glob(..., &method(:require))` -> `Dir.glob(...).sort.each(&method(:require))`.
        let Some(args) = call.arguments() else { return };
        let list = args.arguments();
        let Some(last_real_arg) = list.last() else { return };
        let remove_span = Span::new(last_real_arg.span().end, block.span().end);
        let mut insertion = b".sort.each(".to_vec();
        insertion.extend_from_slice(ctx.text(block.span()));
        insertion.push(b')');
        ctx.report_with_fix(
            &NonDeterministicRequireOrder::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::delete(remove_span), Edit::insert(span.end, insertion)],
            },
        );
    } else {
        // `Dir[...].each(&method(:require))` -> `Dir[...].sort.each(&method(:require))`.
        let Some(message_loc) = call.message_loc() else { return };
        ctx.report_with_fix(
            &NonDeterministicRequireOrder::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(message_loc.span(), b"sort.each".to_vec())],
            },
        );
    }
}
