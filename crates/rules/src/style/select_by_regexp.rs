//! `Style/SelectByRegexp`, ported from RuboCop's
//! `lib/rubocop/cop/style/select_by_regexp.rb`.
//!
//! Whitequark represents `/regexp with (?<name>...)/ =~ expr` (a `=~` whose
//! literal regexp receiver has named captures) as a distinct
//! `match-with-lvasgn` node, with the same two children (regexp operand,
//! matched expression) as an ordinary `(send regexp :=~ expr)`. Prism gives
//! it its own [`ruby_ast::node::MatchWriteNode`], wrapping an ordinary
//! `=~` [`ruby_ast::node::CallNode`] in its `call` field; [`regexp_call`]
//! unwraps it transparently so every other helper here (which only ever
//! inspects the `=~` call's receiver/argument, never the named-capture
//! writes) needs no separate branch for it.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Prefer `%s` to `%s` with a regexp match.";

/// Prefer `grep`/`grep_v` to `select`/`reject`/`find_all`/`filter` with a
/// regexp match.
#[derive(Debug, Clone)]
pub struct SelectByRegexp;

impl Rule for SelectByRegexp {
    const META: RuleMeta = RuleMeta {
        name: "Style/SelectByRegexp",
        department: Department::Style,
        summary: "Prefer `grep`/`grep_v` to `select`/`reject`/`find_all`/`filter` with a \
            regexp match.",
        explanation: "Looks for places where a subset of an Enumerable (array, range, set, \
            etc.; see note below) is calculated based on a `Regexp` match, and suggests `grep` \
            or `grep_v` instead.\n\nNOTE: Hashes do not behave as you may expect with `grep`, \
            which means that `hash.grep` is not equivalent to `hash.select`. Although RuboCop \
            is limited by static analysis, this cop attempts to avoid registering an offense \
            when the receiver is a hash (hash literal, `Hash.new`, `Hash#[]`, or \
            `to_h`/`to_hash`).\n\nNOTE: `grep` and `grep_v` were optimized when used without a \
            block in Ruby 3.0, but may be slower in previous versions.\n\nAutocorrection is \
            marked as unsafe because `MatchData` will not be created by `grep`, but may have \
            previously been relied upon after the `match?` or `=~` call. Additionally, the cop \
            cannot guarantee that the receiver of `select` or `reject` is actually an array by \
            static analysis, so the correction may not be actually equivalent.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
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
        let method_name = call.name().as_slice();
        let is_select = matches!(method_name, b"select" | b"filter" | b"find_all");
        if !is_select && method_name != b"reject" {
            return;
        }
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
        let Some(body) = single_statement(&block) else { return };
        if call.receiver().is_some_and(|r| receiver_allowed(&r)) {
            return;
        }
        let Some(var_name) = block_var_name(&block) else { return };
        let Some(regexp_check) = extract_regexp_check(&body, &var_name) else { return };
        if match_predicate_without_receiver(&regexp_check) {
            return;
        }

        let negated = is_negated(&regexp_check);
        let grep_method = match (is_select, negated) {
            (true, false) | (false, true) => "grep",
            (true, true) | (false, false) => "grep_v",
        };
        let Some(regexp_literal) = find_regexp(&regexp_check, &var_name, ctx) else { return };

        let method_str = String::from_utf8_lossy(method_name);
        let message = MSG.replacen("%s", grep_method, 1).replacen("%s", &method_str, 1);

        let Some(message_loc) = call.message_loc() else { return };
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(
                    Span::new(message_loc.span().start, node.span().end),
                    format!("{grep_method}({regexp_literal})").into_bytes(),
                )],
            },
        );
    }
}

/// RuboCop's `block_node.body&.begin_type?` check inverted.
fn single_statement<'pr>(block: &ruby_ast::node::BlockNode<'pr>) -> Option<Node<'pr>> {
    let body = block.body()?;
    let stmts = body.as_statements_node()?;
    let items = stmts.body();
    if items.len() == 1 {
        items.first()
    } else {
        None
    }
}

/// RuboCop's `receiver_allowed?`.
fn receiver_allowed(receiver: &Node<'_>) -> bool {
    receiver.as_hash_node().is_some() || is_env_const(receiver) || creates_hash(receiver)
}

/// RuboCop's `env_const?`: `(const {nil? cbase} :ENV)`.
fn is_env_const(node: &Node<'_>) -> bool {
    const_named(node, b"ENV")
}

/// Whether `node` is a (possibly namespaced) constant named `name`.
fn const_named(node: &Node<'_>, name: &[u8]) -> bool {
    match node {
        Node::ConstantReadNode { .. } => {
            node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == name)
        }
        Node::ConstantPathNode { .. } => node
            .as_constant_path_node()
            .is_some_and(|p| p.name().is_some_and(|n| n.as_slice() == name)),
        _ => false,
    }
}

/// RuboCop's `creates_hash?`.
fn creates_hash(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    match call.name().as_slice() {
        b"new" | b"[]" => call.receiver().is_some_and(|r| const_named(&r, b"Hash")),
        b"to_h" | b"to_hash" => true,
        _ => false,
    }
}

/// RuboCop's `regexp_match?`'s captured block argument name. See
/// `Style/SelectByKind`'s identical `block_var_name`.
fn block_var_name(block: &ruby_ast::node::BlockNode<'_>) -> Option<Vec<u8>> {
    match block.parameters()? {
        Node::NumberedParametersNode { .. } => {
            let n = block.parameters()?.as_numbered_parameters_node()?;
            (n.maximum() == 1).then(|| b"_1".to_vec())
        }
        Node::ItParametersNode { .. } => Some(b"it".to_vec()),
        Node::BlockParametersNode { .. } => {
            let p = block.parameters()?.as_block_parameters_node()?;
            let params = p.parameters()?;
            if params.requireds().len() == 1
                && params.optionals().is_empty()
                && params.rest().is_none()
                && params.posts().is_empty()
                && params.keywords().is_empty()
                && params.keyword_rest().is_none()
                && params.block().is_none()
            {
                let req = params.requireds().first()?;
                Some(req.as_required_parameter_node()?.name().as_slice().to_vec())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// RuboCop's `unwrap_negation`, including its `receiver.begin_type?`
/// paren-unwrap.
fn unwrap_negation<'pr>(node: &Node<'pr>) -> Node<'pr> {
    if let Some(call) = node.as_call_node() {
        if call.name().as_slice() == b"!" {
            if let Some(receiver) = call.receiver() {
                return unwrap_parens(&receiver);
            }
        }
    }
    *node
}

/// Unwraps a single-statement `ParenthesesNode`, e.g. `(x =~ /re/)` ->
/// `x =~ /re/`.
fn unwrap_parens<'pr>(node: &Node<'pr>) -> Node<'pr> {
    let Some(parens) = node.as_parentheses_node() else { return *node };
    let Some(stmts) = parens.body().and_then(|b| b.as_statements_node()) else { return *node };
    let body = stmts.body();
    if body.len() == 1 {
        body.first().unwrap_or(*node)
    } else {
        *node
    }
}

/// Whether `node` reads the block's parameter by name.
fn reads_var(node: &Node<'_>, var_name: &[u8]) -> bool {
    match node {
        Node::LocalVariableReadNode { .. } => {
            node.as_local_variable_read_node().is_some_and(|n| n.name().as_slice() == var_name)
        }
        Node::ItLocalVariableReadNode { .. } => var_name == b"it",
        _ => false,
    }
}

/// Unwraps RuboCop's `match-with-lvasgn` to its plain `=~` call (see module
/// doc), transparent to a node that is already an ordinary call.
fn regexp_call<'pr>(node: &Node<'pr>) -> Option<ruby_ast::node::CallNode<'pr>> {
    if let Some(call) = node.as_call_node() {
        return Some(call);
    }
    node.as_match_write_node().map(|m| m.call())
}

/// RuboCop's `extract_send_node`/`calls_lvar?`: the body's `match?`/`=~`/
/// `!~` check (or its `!`-negation), re-verified to read the block's own
/// parameter as either the receiver or the (sole) argument.
fn extract_regexp_check<'pr>(body: &Node<'pr>, var_name: &[u8]) -> Option<Node<'pr>> {
    let inner = unwrap_negation(body);
    let call = regexp_call(&inner)?;
    if !matches!(call.name().as_slice(), b"match?" | b"=~" | b"!~") {
        return None;
    }
    let args = call.arguments().map(|a| a.arguments())?;
    if args.len() != 1 {
        return None;
    }
    let receiver_matches = call.receiver().is_some_and(|r| reads_var(&r, var_name));
    let arg_matches = reads_var(&args.first().expect("checked len == 1"), var_name);
    if receiver_matches || arg_matches {
        Some(*body)
    } else {
        None
    }
}

/// RuboCop's `negated?`.
fn is_negated(body: &Node<'_>) -> bool {
    body.as_call_node().is_some_and(|c| matches!(c.name().as_slice(), b"!" | b"!~"))
}

/// RuboCop's `match_predicate_without_receiver?`.
fn match_predicate_without_receiver(body: &Node<'_>) -> bool {
    let inner = unwrap_negation(body);
    let Some(call) = regexp_call(&inner) else { return false };
    call.name().as_slice() == b"match?" && call.receiver().is_none()
}

/// RuboCop's `find_regexp`.
fn find_regexp(body: &Node<'_>, var_name: &[u8], ctx: &Context<'_>) -> Option<String> {
    let inner = unwrap_negation(body);
    let call = regexp_call(&inner)?;
    let receiver = call.receiver();
    if receiver.as_ref().is_some_and(|r| reads_var(r, var_name)) {
        let arg = call.arguments()?.arguments().first()?;
        Some(String::from_utf8_lossy(ctx.text(arg.span())).into_owned())
    } else if call
        .arguments()
        .and_then(|a| a.arguments().first())
        .is_some_and(|a| reads_var(&a, var_name))
    {
        Some(String::from_utf8_lossy(ctx.text(receiver?.span())).into_owned())
    } else {
        None
    }
}
