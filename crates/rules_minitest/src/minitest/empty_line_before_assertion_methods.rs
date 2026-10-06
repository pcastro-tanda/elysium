//! `Minitest/EmptyLineBeforeAssertionMethods`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/empty_line_before_assertion_methods.rb` (with its
//! `MinitestExplorationHelpers` mixin).
//!
//! The upstream cop reads `left_sibling` and `parent` of whitequark nodes, so
//! this port walks the tree itself and hands every node the whitequark left
//! sibling it would have (see [`walk`]).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::is_heredoc;
use ruby_ast::node::CallNode;
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _};
use ruby_source::Span;

const MSG: &str = "Add empty line before assertion.";

const MATCHER_METHODS: &[&[u8]] = &[
    b"must_be_empty",
    b"must_equal",
    b"must_be_close_to",
    b"must_be_within_delta",
    b"must_be_within_epsilon",
    b"must_include",
    b"must_be_instance_of",
    b"must_be_kind_of",
    b"must_match",
    b"must_be_nil",
    b"must_be",
    b"must_respond_to",
    b"must_be_same_as",
    b"path_must_exist",
    b"path_wont_exist",
    b"wont_be_empty",
    b"wont_equal",
    b"wont_be_close_to",
    b"wont_be_within_delta",
    b"wont_be_within_epsilon",
    b"wont_include",
    b"wont_be_instance_of",
    b"wont_be_kind_of",
    b"wont_match",
    b"wont_be_nil",
    b"wont_be",
    b"wont_respond_to",
    b"wont_be_same_as",
    b"must_output",
    b"must_pattern_match",
    b"must_raise",
    b"must_be_silent",
    b"must_throw",
    b"wont_pattern_match",
];

/// Enforces empty line before assertion methods because it separates
/// assertion phase.
#[derive(Debug, Clone)]
pub struct EmptyLineBeforeAssertionMethods;

impl Rule for EmptyLineBeforeAssertionMethods {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/EmptyLineBeforeAssertionMethods",
        department: Department::Minitest,
        summary: "Add empty line before assertion methods.",
        explanation: "Enforces empty line before assertion methods because it separates \
                      assertion phase.\n\n```ruby\n# bad\ndo_something\n\
                      assert_equal(expected, actual)\n\n# good\ndo_something\n\n\
                      assert_equal(expected, actual)\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        walk(&root, None, ctx);
    }
}

/// The whitequark `left_sibling` of a node: a node of the tree, or one that
/// Prism has no node for (an `ivasgn` target, an `mlhs`, a `rescue` chain, ..)
/// of which only the extent matters.
#[derive(Clone, Copy)]
enum Prev<'pr> {
    Node(Node<'pr>),
    Span(Span),
}

/// Visits `node`; `prev` is its whitequark `left_sibling` when that sibling
/// is a node the cop can act on. Everything else upstream returns on is
/// `None`: no sibling, a non-node sibling (a method name, an operator
/// symbol, a missing superclass), an `args` sibling, a parent that is
/// `if`/`while`/`until` (`basic_conditional?`) or a `resbody`.
fn walk<'pr>(node: &Node<'pr>, prev: Option<Prev<'pr>>, ctx: &mut Context<'_>) {
    check(node, prev, ctx);

    if let Some(statements) = node.as_statements_node() {
        // More than one statement is a `begin` node; a lone one takes the
        // sibling its parent gives the whole body.
        let body: Vec<Node<'pr>> = statements.body().iter().collect();
        for (i, statement) in body.iter().enumerate() {
            let sibling =
                if body.len() >= 2 { i.checked_sub(1).map(|j| Prev::Node(body[j])) } else { prev };
            walk(statement, sibling, ctx);
        }
    } else if let Some(else_node) = node.as_else_node() {
        if let Some(statements) = else_node.statements() {
            walk(&statements.as_node(), prev, ctx);
        }
    } else if let Some(ensure) = node.as_ensure_node() {
        if let Some(statements) = ensure.statements() {
            walk(&statements.as_node(), prev, ctx);
        }
    } else if let Some(begin) = node.as_begin_node() {
        walk_begin(&begin, ctx);
    } else if let Some(class) = node.as_class_node() {
        walk(&class.constant_path(), None, ctx);
        let superclass = class.superclass();
        if let Some(superclass) = &superclass {
            walk(superclass, None, ctx);
        }
        if let Some(body) = class.body() {
            walk(&body, superclass.map(Prev::Node), ctx);
        }
    } else if let Some(module) = node.as_module_node() {
        let name = module.constant_path();
        walk(&name, None, ctx);
        if let Some(body) = module.body() {
            walk(&body, Some(Prev::Node(name)), ctx);
        }
    } else if let Some(sclass) = node.as_singleton_class_node() {
        let expression = sclass.expression();
        walk(&expression, None, ctx);
        if let Some(body) = sclass.body() {
            walk(&body, Some(Prev::Node(expression)), ctx);
        }
    } else if let Some(for_node) = node.as_for_node() {
        let collection = for_node.collection();
        walk(&for_node.index(), None, ctx);
        walk(&collection, None, ctx);
        if let Some(statements) = for_node.statements() {
            walk(&statements.as_node(), Some(Prev::Node(collection)), ctx);
        }
    } else if let Some(when) = node.as_when_node() {
        let mut last = None;
        for condition in &when.conditions() {
            walk(&condition, last.map(Prev::Node), ctx);
            last = Some(condition);
        }
        if let Some(statements) = when.statements() {
            walk(&statements.as_node(), last.map(Prev::Node), ctx);
        }
    } else if let Some(in_node) = node.as_in_node() {
        // `(in_pattern pattern guard body)`: only a guard (an `if`/`unless`
        // pattern in Prism) is a sibling node of the body.
        let pattern = in_node.pattern();
        walk(&pattern, None, ctx);
        if let Some(statements) = in_node.statements() {
            let guard = (pattern.as_if_node().is_some() || pattern.as_unless_node().is_some())
                .then_some(Prev::Node(pattern));
            walk(&statements.as_node(), guard, ctx);
        }
    } else if let Some(case) = node.as_case_node() {
        let conditions: Vec<Node<'pr>> = case.conditions().iter().collect();
        walk_case(case.predicate(), &conditions, case.else_clause(), ctx);
    } else if let Some(case) = node.as_case_match_node() {
        let conditions: Vec<Node<'pr>> = case.conditions().iter().collect();
        walk_case(case.predicate(), &conditions, case.else_clause(), ctx);
    } else if let Some(range) = node.as_range_node() {
        let left = range.left();
        if let Some(left) = &left {
            walk(left, None, ctx);
        }
        if let Some(right) = range.right() {
            walk(&right, left.map(Prev::Node), ctx);
        }
    } else if walk_writes(node, ctx) {
        // Handled.
    } else if node.as_arguments_node().is_some()
        || node.as_array_node().is_some()
        || node.as_and_node().is_some()
        || node.as_or_node().is_some()
        || node.as_assoc_node().is_some()
    {
        // The children are siblings in source order.
        let mut last: Option<Node<'pr>> = None;
        for_each_child(node, |child| {
            walk(child, last.map(Prev::Node), ctx);
            last = Some(*child);
        });
    } else {
        for_each_child(node, |child| walk(child, None, ctx));
    }
}

/// `or_asgn`/`and_asgn`/`masgn`: the value's left sibling is the target.
/// False when `node` is none of those.
fn walk_writes(node: &Node<'_>, ctx: &mut Context<'_>) -> bool {
    if let Some((target, value)) = or_and_write(node) {
        // `(or_asgn target value)`: the target is a node.
        let value = value.span();
        for_each_child(node, |child| {
            let sibling = (child.span() == value).then_some(Prev::Span(target));
            walk(child, sibling, ctx);
        });
    } else if let Some(multi) = node.as_multi_write_node() {
        // `(masgn mlhs value)`
        let target = multi_target_span(&multi);
        let value = multi.value().span();
        for_each_child(node, |child| {
            let sibling = (child.span() == value).then_some(Prev::Span(target));
            walk(child, sibling, ctx);
        });
    } else {
        return false;
    }
    true
}

/// `case`/`case_match`: the `else` body's left sibling is the last
/// `when`/`in`.
fn walk_case<'pr>(
    predicate: Option<Node<'pr>>,
    conditions: &[Node<'pr>],
    else_clause: Option<ruby_ast::node::ElseNode<'pr>>,
    ctx: &mut Context<'_>,
) {
    if let Some(predicate) = &predicate {
        walk(predicate, None, ctx);
    }
    for condition in conditions {
        walk(condition, None, ctx);
    }
    if let Some(else_clause) = else_clause {
        let last = conditions.last().map(|c| Prev::Span(c.span()));
        walk(&else_clause.as_node(), last, ctx);
    }
}

/// `(kwbegin (ensure (rescue body resbody else) ensure_body))`: the `else`
/// body follows the last `resbody`, the `ensure` body follows the rescue
/// chain (or the body when there is none).
fn walk_begin<'pr>(begin: &ruby_ast::node::BeginNode<'pr>, ctx: &mut Context<'_>) {
    let statements = begin.statements();
    if let Some(statements) = &statements {
        walk(&statements.as_node(), None, ctx);
    }
    let rescue = begin.rescue_clause();
    let mut last_resbody = None;
    let mut chain_end = None;
    if let Some(rescue) = &rescue {
        walk(&rescue.as_node(), None, ctx);
        let mut current = *rescue;
        loop {
            last_resbody = Some(rescue_extent(&current));
            match current.subsequent() {
                Some(next) => current = next,
                None => break,
            }
        }
        chain_end = last_resbody.map(|span: Span| span.end);
    }
    let else_clause = begin.else_clause();
    if let Some(else_clause) = &else_clause {
        walk(&else_clause.as_node(), last_resbody.map(Prev::Span), ctx);
        let end = else_clause
            .statements()
            .map_or(else_clause.else_keyword_loc().span().end, |s| s.location().span().end);
        chain_end = Some(chain_end.map_or(end, |e| e.max(end)));
    }
    if let Some(ensure) = begin.ensure_clause() {
        let body_prev = if let Some(rescue) = &rescue {
            let start = statements
                .as_ref()
                .map_or(rescue.keyword_loc().span().start, |s| s.location().span().start);
            chain_end.map(|end| Prev::Span(Span::new(start, end)))
        } else if let Some(statements) = &statements {
            let body: Vec<Node<'pr>> = statements.body().iter().collect();
            match body.as_slice() {
                [] => None,
                [only] => Some(Prev::Node(*only)),
                _ => Some(Prev::Span(statements.location().span())),
            }
        } else {
            None
        };
        walk(&ensure.as_node(), body_prev, ctx);
    }
}

/// The extent of one `resbody`.
fn rescue_extent(rescue: &ruby_ast::node::RescueNode<'_>) -> Span {
    let start = rescue.keyword_loc().span().start;
    let mut end = rescue.keyword_loc().span().end;
    for exception in &rescue.exceptions() {
        end = end.max(exception.span().end);
    }
    if let Some(reference) = rescue.reference() {
        end = end.max(reference.span().end);
    }
    if let Some(statements) = rescue.statements() {
        end = end.max(statements.location().span().end);
    }
    Span::new(start, end)
}

macro_rules! target_and_value {
    ($node:expr, name: [$($name_accessor:ident),+ $(,)?], path: [$($path_accessor:ident),+ $(,)?]) => {
        $(
            if let Some(n) = $node.$name_accessor() {
                return Some((n.name_loc().span(), n.value()));
            }
        )+
        $(
            if let Some(n) = $node.$path_accessor() {
                return Some((n.target().location().span(), n.value()));
            }
        )+
    };
}

/// `(or_asgn target value)` / `(and_asgn target value)`: the target's extent
/// and the value.
fn or_and_write<'pr>(node: &Node<'pr>) -> Option<(Span, Node<'pr>)> {
    target_and_value!(
        node,
        name: [
            as_local_variable_or_write_node,
            as_instance_variable_or_write_node,
            as_class_variable_or_write_node,
            as_global_variable_or_write_node,
            as_constant_or_write_node,
            as_local_variable_and_write_node,
            as_instance_variable_and_write_node,
            as_class_variable_and_write_node,
            as_global_variable_and_write_node,
            as_constant_and_write_node,
        ],
        path: [as_constant_path_or_write_node, as_constant_path_and_write_node]
    );
    // `recv.name ||= value`: the `send` target ends at the method name.
    let call =
        node.as_call_or_write_node().map(|n| (n.receiver(), n.message_loc(), n.value())).or_else(
            || node.as_call_and_write_node().map(|n| (n.receiver(), n.message_loc(), n.value())),
        );
    if let Some((receiver, message, value)) = call {
        let message = message?.span();
        let start = receiver.map_or(message.start, |r| r.span().start);
        return Some((Span::new(start, message.end), value));
    }
    // `recv[args] ||= value`: the `send` target ends at `]`.
    let index = node
        .as_index_or_write_node()
        .map(|n| (n.receiver(), n.opening_loc().span(), n.closing_loc().span(), n.value()))
        .or_else(|| {
            node.as_index_and_write_node()
                .map(|n| (n.receiver(), n.opening_loc().span(), n.closing_loc().span(), n.value()))
        });
    let (receiver, opening, closing, value) = index?;
    let start = receiver.map_or(opening.start, |r| r.span().start);
    Some((Span::new(start, closing.end), value))
}

/// The `mlhs` of a `masgn`.
fn multi_target_span(multi: &ruby_ast::node::MultiWriteNode<'_>) -> Span {
    let mut start = u32::MAX;
    let mut end = 0;
    let mut extend = |span: Span| {
        start = start.min(span.start);
        end = end.max(span.end);
    };
    for target in &multi.lefts() {
        extend(target.span());
    }
    if let Some(rest) = multi.rest() {
        extend(rest.span());
    }
    for target in &multi.rights() {
        extend(target.span());
    }
    if let Some(lparen) = multi.lparen_loc() {
        extend(lparen.span());
    }
    if let Some(rparen) = multi.rparen_loc() {
        extend(rparen.span());
    }
    Span::new(start, end)
}

/// `on_send`.
fn check<'pr>(node: &Node<'pr>, prev: Option<Prev<'pr>>, ctx: &mut Context<'_>) {
    let Some(prev) = prev else { return };
    if !is_assertion_candidate(node) {
        return;
    }
    let (previous_line, offset) = match prev {
        // A sibling Prism has no node for: never an assertion, a heredoc or
        // a block.
        Prev::Span(span) => (ctx.last_line(span), None),
        Prev::Node(previous_line_node) => {
            // `accept_previous_line?`
            if assertion_method_p(&previous_line_node) {
                return;
            }
            let previous_line_node =
                heredoc_last_argument(&previous_line_node).unwrap_or(previous_line_node);
            if use_assertion_method_at_last_of_block(&previous_line_node) {
                return;
            }
            match heredoc_end_offset(&previous_line_node) {
                Some(offset) => (ctx.line_col(offset).line, Some(offset)),
                None => (ctx.last_line(previous_line_node.span()), None),
            }
        }
    };

    // `no_empty_line?`
    if previous_line + 1 != ctx.line_col(node.span().start).line {
        return;
    }

    // `register_offense`
    let offset = offset.unwrap_or_else(|| {
        // `range_by_whole_lines(.., include_final_newline: true)`.
        let line = ctx.line_span(previous_line).end;
        if ctx.source().bytes().get(line as usize) == Some(&b'\n') {
            line + 1
        } else {
            line
        }
    });
    ctx.report_with_fix(
        &EmptyLineBeforeAssertionMethods::META,
        node.span(),
        MSG,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::insert(offset, b"\n".to_vec())],
        },
    );
}

/// `assertion_method(node)` for the call (or lambda) `node`: it is itself the
/// assertion, or the block wrapping a lone assertion.
fn is_assertion_candidate(node: &Node<'_>) -> bool {
    if let Some(call) = node.as_call_node() {
        // `on_send` does not fire for `csend`.
        if call.is_safe_navigation() {
            return false;
        }
        let block = block_node(&call);
        if assertion_method_name(&call) {
            // As the `send` of a block it is the block's first child, so it
            // has no left sibling.
            return block.is_none();
        }
        let Some(block) = block else { return false };
        // `parent.block_type?`, `!parent.method?(:test)`
        if !is_plain_block(block.parameters()) || call.name().as_slice() == b"test" {
            return false;
        }
        return lone_statement(block.body()).is_some_and(|body| assertion_method_p(&body));
    }
    if let Some(lambda) = node.as_lambda_node() {
        // A legacy-mode lambda literal is `(block (send nil :lambda) ..)`.
        return is_plain_block(lambda.parameters())
            && lone_statement(lambda.body()).is_some_and(|body| assertion_method_p(&body));
    }
    false
}

/// `parent.body` when that is a single statement (a `begin` otherwise).
fn lone_statement(body: Option<Node<'_>>) -> Option<Node<'_>> {
    let body = body?;
    match body.as_statements_node() {
        Some(statements) => {
            let mut iter = statements.body().iter();
            let first = iter.next()?;
            iter.next().is_none().then_some(first)
        }
        None => Some(body),
    }
}

/// `block_type?` (not `numblock`/`itblock`).
fn is_plain_block(parameters: Option<Node<'_>>) -> bool {
    // `numblock` and `itblock` are not `block_type?`.
    parameters.is_none_or(|p| {
        p.as_numbered_parameters_node().is_none() && p.as_it_parameters_node().is_none()
    })
}

fn block_node<'pr>(call: &CallNode<'pr>) -> Option<ruby_ast::node::BlockNode<'pr>> {
    call.block()?.as_block_node()
}

/// `assertion_prefix_method?(node) || node.method?(:flunk) ||
/// MATCHER_METHODS.include?(node.method_name)`.
fn assertion_method_name(call: &CallNode<'_>) -> bool {
    let name = call.name();
    let name = name.as_slice();
    if call.receiver().is_none() && (name.starts_with(b"assert") || name.starts_with(b"refute")) {
        return true;
    }
    name == b"flunk" || MATCHER_METHODS.contains(&name)
}

/// `assertion_method?(node)`.
fn assertion_method_p(node: &Node<'_>) -> bool {
    if let Some(call) = node.as_call_node() {
        // `type?(:send, :any_block)`: a `csend` without a block is neither.
        if call.is_safe_navigation() && block_node(&call).is_none() {
            return false;
        }
        return assertion_method_name(&call);
    }
    match assignment_value(node) {
        Some(value) => assertion_method_p(&value),
        None => false,
    }
}

macro_rules! first_value {
    ($node:expr, $($accessor:ident),+ $(,)?) => {
        $(
            if let Some(n) = $node.$accessor() {
                return Some(n.value());
            }
        )+
    };
}

/// `node.expression` of an `assignment?` node.
fn assignment_value<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    first_value!(
        node,
        as_local_variable_write_node,
        as_instance_variable_write_node,
        as_class_variable_write_node,
        as_global_variable_write_node,
        as_constant_write_node,
        as_constant_path_write_node,
        as_multi_write_node,
        as_local_variable_operator_write_node,
        as_instance_variable_operator_write_node,
        as_class_variable_operator_write_node,
        as_global_variable_operator_write_node,
        as_constant_operator_write_node,
        as_constant_path_operator_write_node,
        as_call_operator_write_node,
        as_index_operator_write_node,
        as_local_variable_or_write_node,
        as_instance_variable_or_write_node,
        as_class_variable_or_write_node,
        as_global_variable_or_write_node,
        as_constant_or_write_node,
        as_constant_path_or_write_node,
        as_call_or_write_node,
        as_index_or_write_node,
        as_local_variable_and_write_node,
        as_instance_variable_and_write_node,
        as_class_variable_and_write_node,
        as_global_variable_and_write_node,
        as_constant_and_write_node,
        as_constant_path_and_write_node,
        as_call_and_write_node,
        as_index_and_write_node,
    );
    None
}

/// `use_heredoc_argument?(node)`: the node's last argument when it is a
/// heredoc.
fn heredoc_last_argument<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    let last = if let Some(call) = node.as_call_node() {
        // A call with a literal block is a `block`, whose `arguments` are
        // the block parameters.
        if block_node(&call).is_some() {
            return None;
        }
        let mut last = call.arguments().and_then(|args| args.arguments().iter().last());
        if let Some(block_arg) = call.block() {
            last = Some(block_arg);
        }
        last
    } else if let Some(sup) = node.as_super_node() {
        if sup.block().is_some_and(|b| b.as_block_node().is_some()) {
            return None;
        }
        let mut last = sup.arguments().and_then(|args| args.arguments().iter().last());
        if let Some(block_arg) = sup.block() {
            last = Some(block_arg);
        }
        last
    } else if let Some(yield_node) = node.as_yield_node() {
        yield_node.arguments().and_then(|args| args.arguments().iter().last())
    } else if let Some(ret) = node.as_return_node() {
        ret.arguments().and_then(|args| args.arguments().iter().last())
    } else if let Some(brk) = node.as_break_node() {
        brk.arguments().and_then(|args| args.arguments().iter().last())
    } else if let Some(next) = node.as_next_node() {
        next.arguments().and_then(|args| args.arguments().iter().last())
    } else {
        None
    }?;
    is_heredoc(&last).then_some(last)
}

/// `use_assertion_method_at_last_of_block?(node)`.
fn use_assertion_method_at_last_of_block(node: &Node<'_>) -> bool {
    let body = if let Some(call) = node.as_call_node() {
        let Some(block) = block_node(&call) else { return false };
        if !is_plain_block(block.parameters()) {
            return false;
        }
        block.body()
    } else if let Some(lambda) = node.as_lambda_node() {
        if !is_plain_block(lambda.parameters()) {
            return false;
        }
        lambda.body()
    } else if let Some(sup) = node.as_super_node() {
        let Some(block) = sup.block().and_then(|b| b.as_block_node()) else { return false };
        if !is_plain_block(block.parameters()) {
            return false;
        }
        block.body()
    } else if let Some(sup) = node.as_forwarding_super_node() {
        let Some(block) = sup.block() else { return false };
        if !is_plain_block(block.parameters()) {
            return false;
        }
        block.body()
    } else {
        return false;
    };
    let Some(body) = body else { return false };
    match body.as_statements_node() {
        // `begin`: its last child; otherwise the lone statement.
        Some(statements) => {
            statements.body().iter().last().is_some_and(|last| assertion_method_p(&last))
        }
        None => assertion_method_p(&body),
    }
}

/// `loc.heredoc_end` end offset of a heredoc string: the terminator without
/// its newline.
fn heredoc_end_offset(node: &Node<'_>) -> Option<u32> {
    if !is_heredoc(node) {
        return None;
    }
    let closing = if let Some(n) = node.as_string_node() {
        n.closing_loc()
    } else if let Some(n) = node.as_interpolated_string_node() {
        n.closing_loc()
    } else if let Some(n) = node.as_x_string_node() {
        Some(n.closing_loc())
    } else {
        node.as_interpolated_x_string_node().map(|n| n.closing_loc())
    }?;
    let span: Span = closing.span();
    let slice = closing.as_slice();
    let trimmed = slice.strip_suffix(b"\n").unwrap_or(slice);
    let trimmed = trimmed.strip_suffix(b"\r").unwrap_or(trimmed);
    Some(span.start + u32::try_from(trimmed.len()).expect("offset exceeds u32"))
}
