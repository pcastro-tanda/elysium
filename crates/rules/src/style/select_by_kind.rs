//! `Style/SelectByKind`, ported from RuboCop's
//! `lib/rubocop/cop/style/select_by_kind.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Prefer `%s` to `%s` with a kind check.";

/// Prefer `grep`/`grep_v` to `select`/`reject`/`find_all`/`filter` with a
/// kind check.
#[derive(Debug, Clone)]
pub struct SelectByKind;

impl Rule for SelectByKind {
    const META: RuleMeta = RuleMeta {
        name: "Style/SelectByKind",
        department: Department::Style,
        summary: "Prefer `grep`/`grep_v` to `select`/`reject`/`find_all`/`filter` with a kind \
            check.",
        explanation: "Looks for places where a subset of an Enumerable (array, range, set, \
            etc.; see note below) is calculated based on a class type check, and suggests \
            `grep` or `grep_v` instead.\n\nNOTE: Hashes do not behave as you may expect with \
            `grep`, which means that `hash.grep` is not equivalent to `hash.select`. Although \
            RuboCop is limited by static analysis, this cop attempts to avoid registering an \
            offense when the receiver is a hash (hash literal, `Hash.new`, `Hash#[]`, or \
            `to_h`/`to_hash`).\n\nAutocorrection is marked as unsafe because the cop cannot \
            guarantee that the receiver is actually an array by static analysis, so the \
            correction may not be actually equivalent.",
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
        let Some(class_check) = extract_class_check(&body, &var_name) else { return };

        let negated = class_check.as_call_node().is_some_and(|c| c.name().as_slice() == b"!");
        let replacement = match (is_select, negated) {
            (true, false) | (false, true) => "grep",
            (true, true) | (false, false) => "grep_v",
        };
        let Some(class_constant) = find_class_constant(&class_check) else { return };

        let method_str = String::from_utf8_lossy(method_name);
        let message = MSG.replacen("%s", replacement, 1).replacen("%s", &method_str, 1);

        let Some(message_loc) = call.message_loc() else { return };
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(
                    Span::new(message_loc.span().start, node.span().end),
                    [replacement.as_bytes(), b"(", ctx.text(class_constant.span()), b")"].concat(),
                )],
            },
        );
    }
}

/// RuboCop's `block_node.body&.begin_type?` check inverted: the block's
/// single statement, or `None` for an empty or multi-statement body.
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

/// RuboCop's `class_check?`'s captured block argument name: the single
/// required parameter's name, `_1` for a `maximum == 1` numbered block, or
/// `it` for an `itblock`. `None` for any other arity/shape (RuboCop's
/// `(args (arg $_))`/`$1`/`$_` captures all require exactly one).
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

/// RuboCop's `unwrap_negation`.
fn unwrap_negation<'pr>(node: &Node<'pr>) -> Node<'pr> {
    if let Some(call) = node.as_call_node() {
        if call.name().as_slice() == b"!" {
            if let Some(receiver) = call.receiver() {
                return receiver;
            }
        }
    }
    *node
}

/// Whether `node` reads the block's parameter by name (a plain local
/// variable, a numbered `_1`, or an `it` implicit parameter).
fn reads_var(node: &Node<'_>, var_name: &[u8]) -> bool {
    match node {
        Node::LocalVariableReadNode { .. } => {
            node.as_local_variable_read_node().is_some_and(|n| n.name().as_slice() == var_name)
        }
        Node::ItLocalVariableReadNode { .. } => var_name == b"it",
        _ => false,
    }
}

/// RuboCop's `extract_send_node`: the body's `is_a?`/`kind_of?` check (or
/// its `!`-negation), re-verified to read the block's own parameter.
fn extract_class_check<'pr>(body: &Node<'pr>, var_name: &[u8]) -> Option<Node<'pr>> {
    let inner = unwrap_negation(body);
    let call = inner.as_call_node()?;
    if !matches!(call.name().as_slice(), b"is_a?" | b"kind_of?") {
        return None;
    }
    let receiver = call.receiver()?;
    if !reads_var(&receiver, var_name) {
        return None;
    }
    Some(*body)
}

/// RuboCop's `find_class_constant`.
fn find_class_constant<'pr>(class_check: &Node<'pr>) -> Option<Node<'pr>> {
    let inner = unwrap_negation(class_check);
    let call = inner.as_call_node()?;
    call.arguments()?.arguments().first()
}
