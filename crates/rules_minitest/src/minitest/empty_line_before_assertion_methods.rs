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

/// Visits `node`; `prev` is its whitequark `left_sibling` when that sibling
/// is a node the cop can act on. Everything else upstream returns on is
/// `None`: no sibling, a non-node sibling (a method name, a missing
/// superclass), an `args` sibling, a parent that is `if`/`while`/`until`
/// (`basic_conditional?`) or a `resbody`.
fn walk<'pr>(node: &Node<'pr>, prev: Option<Node<'pr>>, ctx: &mut Context<'_>) {
    check(node, prev, ctx);

    if let Some(statements) = node.as_statements_node() {
        // More than one statement is a `begin` node; a lone one takes the
        // sibling its parent gives the whole body.
        let body: Vec<Node<'pr>> = statements.body().iter().collect();
        for (i, statement) in body.iter().enumerate() {
            let sibling = if body.len() >= 2 { i.checked_sub(1).map(|j| body[j]) } else { prev };
            walk(statement, sibling, ctx);
        }
    } else if let Some(class) = node.as_class_node() {
        walk(&class.constant_path(), None, ctx);
        let superclass = class.superclass();
        if let Some(superclass) = &superclass {
            walk(superclass, None, ctx);
        }
        if let Some(body) = class.body() {
            walk(&body, superclass, ctx);
        }
    } else if let Some(module) = node.as_module_node() {
        let name = module.constant_path();
        walk(&name, None, ctx);
        if let Some(body) = module.body() {
            walk(&body, Some(name), ctx);
        }
    } else if let Some(sclass) = node.as_singleton_class_node() {
        let expression = sclass.expression();
        walk(&expression, None, ctx);
        if let Some(body) = sclass.body() {
            walk(&body, Some(expression), ctx);
        }
    } else if let Some(for_node) = node.as_for_node() {
        let collection = for_node.collection();
        walk(&for_node.index(), None, ctx);
        walk(&collection, None, ctx);
        if let Some(statements) = for_node.statements() {
            walk(&statements.as_node(), Some(collection), ctx);
        }
    } else if let Some(when) = node.as_when_node() {
        let mut last = None;
        for condition in &when.conditions() {
            walk(&condition, None, ctx);
            last = Some(condition);
        }
        if let Some(statements) = when.statements() {
            walk(&statements.as_node(), last, ctx);
        }
    } else if let Some(in_node) = node.as_in_node() {
        // `(in_pattern pattern guard body)`: only a guard (an `if`/`unless`
        // pattern in Prism) is a sibling node of the body.
        let pattern = in_node.pattern();
        walk(&pattern, None, ctx);
        if let Some(statements) = in_node.statements() {
            let guard = (pattern.as_if_node().is_some() || pattern.as_unless_node().is_some())
                .then_some(pattern);
            walk(&statements.as_node(), guard, ctx);
        }
    } else if node.as_arguments_node().is_some()
        || node.as_array_node().is_some()
        || node.as_and_node().is_some()
        || node.as_or_node().is_some()
        || node.as_assoc_node().is_some()
    {
        // The children are siblings in source order.
        let mut last: Option<Node<'pr>> = None;
        for_each_child(node, |child| {
            walk(child, last, ctx);
            last = Some(*child);
        });
    } else {
        for_each_child(node, |child| walk(child, None, ctx));
    }
}

/// `on_send`.
fn check<'pr>(node: &Node<'pr>, prev: Option<Node<'pr>>, ctx: &mut Context<'_>) {
    let Some(previous_line_node) = prev else { return };
    if !is_assertion_candidate(node) {
        return;
    }
    // `accept_previous_line?`
    if assertion_method_p(&previous_line_node) {
        return;
    }
    let previous_line_node =
        heredoc_last_argument(&previous_line_node).unwrap_or(previous_line_node);
    if use_assertion_method_at_last_of_block(&previous_line_node) {
        return;
    }

    // `no_empty_line?`
    let heredoc_end = heredoc_end_offset(&previous_line_node);
    let previous_line = match heredoc_end {
        Some(offset) => ctx.line_col(offset).line,
        None => ctx.last_line(previous_line_node.span()),
    };
    if previous_line + 1 != ctx.line_col(node.span().start).line {
        return;
    }

    // `register_offense`
    let offset = heredoc_end.unwrap_or_else(|| {
        // `range_by_whole_lines(.., include_final_newline: true)`.
        let line = ctx.line_span(ctx.last_line(previous_line_node.span())).end;
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
