//! `Style/SafeNavigation`, ported from RuboCop's
//! `lib/rubocop/cop/style/safe_navigation.rb`.
//!
//! Upstream's whitequark AST represents `unless`/ternary bodies via a shared
//! `if`-node shape (with a literal-`nil` or missing branch) and locates the
//! checked variable's occurrence in the body through the *generic*
//! `Node#receiver` node-matcher (`{(send $_ ...) (any_block (call $_ ...)
//! ...)}`), which exists on every node and simply returns `nil` for a
//! non-matching shape. Prism instead gives `IfNode`/`UnlessNode` their own
//! true/false-branch accessors directly, and a call carrying a block is
//! still one `CallNode` (the block is an attached field, never a wrapping
//! ancestor) -- so [`find_matching_receiver`] collapses to a plain
//! `CallNode::receiver()` walk with no `any_block` case, and the branch
//! extraction in [`if_case`]/[`unless_case`]/[`ternary_case`] reads Prism's
//! real true/false fields instead of replaying whitequark's swapped literal
//! layout.
//!
//! Prism has no parent pointers, so a `(kind, span) -> parent` map is built
//! fresh for each top-level `if`/`unless`/`and` node processed, scoped to
//! that node's own subtree (everywhere [`negated`], [`chain_length`],
//! [`unsafe_method_used`] and [`add_safe_nav_chain_edits`] ever need to
//! climb).
//!
//! `on_and`'s `collect_and_clauses` sliding-window algorithm
//! (`each_slice(2).each_cons(2)` over every operand/operator physically
//! sorted across the whole `&&` chain, including operands borrowed from
//! *nested* `and` nodes not inside a block) is ported literally in
//! [`collect_and_clauses`]: the pairing it produces genuinely differs from
//! "each adjacent original `&&` clause" once the chain nests parens/`||`.

use std::collections::HashMap;
use std::collections::HashSet;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{AndNode, CallNode, IfNode, UnlessNode};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

const MSG: &str = "Use safe navigation (`&.`) instead of checking if an object exists before \
calling the method.";

/// `RuboCop::AST::MethodIdentifierPredicates::OPERATOR_METHODS`.
const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

/// `nil.methods` on Ruby 3.4 plus `NilMethods#other_stdlib_methods` (`to_d`).
/// `AllowedMethods` is merged in at [`SafeNavigation::configure`].
const NIL_METHODS: &[&[u8]] = &[
    b"!",
    b"!=",
    b"!~",
    b"&",
    b"<=>",
    b"==",
    b"===",
    b"=~",
    b"^",
    b"__id__",
    b"__send__",
    b"class",
    b"clone",
    b"define_singleton_method",
    b"display",
    b"dup",
    b"enum_for",
    b"eql?",
    b"equal?",
    b"extend",
    b"freeze",
    b"frozen?",
    b"hash",
    b"inspect",
    b"instance_eval",
    b"instance_exec",
    b"instance_of?",
    b"instance_variable_defined?",
    b"instance_variable_get",
    b"instance_variable_set",
    b"instance_variables",
    b"is_a?",
    b"itself",
    b"kind_of?",
    b"method",
    b"methods",
    b"nil?",
    b"object_id",
    b"private_methods",
    b"protected_methods",
    b"public_method",
    b"public_methods",
    b"public_send",
    b"rationalize",
    b"remove_instance_variable",
    b"respond_to?",
    b"send",
    b"singleton_class",
    b"singleton_method",
    b"singleton_methods",
    b"tap",
    b"then",
    b"to_a",
    b"to_c",
    b"to_d",
    b"to_enum",
    b"to_f",
    b"to_h",
    b"to_i",
    b"to_r",
    b"to_s",
    b"yield_self",
    b"|",
];

/// A `(kind, span) -> parent` map covering one processed node's whole
/// subtree, built fresh per top-level `if`/`unless`/`and` node (Prism has no
/// parent pointers).
type ParentMap<'pr> = HashMap<(NodeKind, Span), Node<'pr>>;

fn build_parent_map<'pr>(root: &Node<'pr>) -> ParentMap<'pr> {
    let mut map = HashMap::new();
    fill_parent_map(root, &mut map);
    map
}

fn fill_parent_map<'pr>(node: &Node<'pr>, map: &mut ParentMap<'pr>) {
    // Whitequark's `(block SEND ARGS BODY)` makes a block's own call a
    // *sibling* of its body, never an ancestor: `BLOCK`'s parent is
    // whatever lies *outside* the whole `SEND do...end` expression, not
    // `SEND` itself. Prism instead attaches the block to its `CallNode` as
    // a field (the same single node represents both whitequark's `SEND`
    // and `BLOCK`), so an unrestricted `each_ancestor` climb starting
    // *inside* the block's body must skip straight from the block to that
    // owning call's own parent -- never stopping at, or counting, the
    // owning call itself at this hop -- to land where whitequark's `BLOCK`
    // ancestor really points. `node`'s own parent is already in `map` by
    // now (inserted by `node`'s caller before recursing here), so the
    // lookup below sees it.
    let block_child = node.as_call_node().and_then(|c| c.block());
    for_each_child(node, |child| {
        let is_block_boundary = block_child.is_some_and(|b| same_position(&b, child));
        if is_block_boundary {
            if let Some(owner_parent) = parent_of(map, node) {
                map.insert((child.kind(), child.span()), owner_parent);
            }
        } else {
            map.insert((child.kind(), child.span()), *node);
        }
        fill_parent_map(child, map);
    });
}

fn parent_of<'pr>(map: &ParentMap<'pr>, node: &Node<'pr>) -> Option<Node<'pr>> {
    map.get(&(node.kind(), node.span())).copied()
}

/// RuboCop's `Node#==` between two `Node` handles for our purposes: same
/// underlying occurrence (Prism hands out fresh values, but never two
/// distinct nodes sharing both kind and span).
fn same_position(a: &Node<'_>, b: &Node<'_>) -> bool {
    a.kind() == b.kind() && a.span() == b.span()
}

/// A `StatementsNode` collapsed to its single statement, standing in for
/// `node_parts`' un-wrapped body; `None` for zero or several statements
/// (upstream's `find_matching_receiver_invocation` never matches a
/// multi-statement `begin`, since it never `respond_to?(:receiver)`).
fn single_body(stmts: Option<ruby_ast::node::StatementsNode<'_>>) -> Option<Node<'_>> {
    let stmts = stmts?;
    let body = stmts.body();
    if body.len() == 1 {
        body.first()
    } else {
        None
    }
}

/// A `StatementsNode` with exactly one statement collapsed to that
/// statement; anything else (including a non-`StatementsNode`) is returned
/// unchanged. Used to look *through* a `ParenthesesNode`'s body.
fn reduce_single_stmt(node: Node<'_>) -> Node<'_> {
    if let Some(stmts) = node.as_statements_node() {
        let body = stmts.body();
        if body.len() == 1 {
            if let Some(one) = body.first() {
                return one;
            }
        }
    }
    node
}

/// RuboCop's `strip_begin`: `(begin $!begin) | $!(begin)` -- one level of
/// parenthesization peeled away (Prism's `ParenthesesNode`, the closest
/// analogue to whitequark's `begin` used for a parenthesized single
/// expression), unless what's inside is *itself* parenthesized.
fn strip_begin(node: Node<'_>) -> Node<'_> {
    if let Some(p) = node.as_parentheses_node() {
        if let Some(body) = p.body() {
            let inner = reduce_single_stmt(body);
            if inner.as_parentheses_node().is_none() {
                return inner;
            }
        }
    }
    node
}

/// RuboCop's `and_inside_begin?`: `` `(begin and ...) `` -- searches `node`
/// and every descendant for a parenthesized `and` expression.
fn and_inside_begin(node: &Node<'_>) -> bool {
    if parens_wrapped_and(node) {
        return true;
    }
    let mut found = false;
    ruby_ast::each_descendant(node, &mut |n| {
        if !found && parens_wrapped_and(n) {
            found = true;
        }
    });
    found
}

fn parens_wrapped_and(node: &Node<'_>) -> bool {
    let Some(p) = node.as_parentheses_node() else { return false };
    let Some(body) = p.body() else { return false };
    reduce_single_stmt(body).as_and_node().is_some()
}

/// RuboCop's `and_with_rhs_or?`: `(and _ {or (begin or)})` -- the *top* `and`
/// node's own (unstripped-by-clause) right-hand side is an `or`, possibly
/// parenthesized.
fn and_with_rhs_or(top: &AndNode<'_>) -> bool {
    is_or_shape(&top.right())
}

fn is_or_shape(node: &Node<'_>) -> bool {
    if node.as_or_node().is_some() {
        return true;
    }
    if let Some(p) = node.as_parentheses_node() {
        if let Some(body) = p.body() {
            return is_or_shape(&reduce_single_stmt(body));
        }
    }
    false
}

/// A call with no arguments and no block -- the shape `.nil?`/`.!`/a bare
/// checked variable all need.
fn as_bare_call<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    if call.arguments().is_some() || call.block().is_some() {
        return None;
    }
    Some(call)
}

/// RuboCop's `not_nil_check?`: `(send (send $_ :nil?) :!)`, i.e. `!X.nil?`.
fn as_not_nil_check<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    let call = as_bare_call(node)?;
    if call.name().as_slice() != b"!" {
        return None;
    }
    let inner = as_bare_call(&call.receiver()?)?;
    if inner.name().as_slice() != b"nil?" {
        return None;
    }
    inner.receiver()
}

/// The shape of an `if`/`unless`/ternary predicate (or an `and` clause's
/// LHS), restated directly against Prism rather than replaying whitequark's
/// `{(send $_ {:nil? :!}) $_}` / `{(send (send $_ :nil?) :!) $_}` groups.
enum CondForm<'pr> {
    /// The predicate itself, unmodified (a plain truthiness check).
    Bare(Node<'pr>),
    /// `X.nil?`.
    NilCheck(Node<'pr>),
    /// `!X`.
    Negated(Node<'pr>),
    /// `!X.nil?`.
    NotNilCheck(Node<'pr>),
}

fn classify_predicate<'pr>(pred: &Node<'pr>) -> CondForm<'pr> {
    if let Some(inner) = as_not_nil_check(pred) {
        return CondForm::NotNilCheck(inner);
    }
    if let Some(call) = as_bare_call(pred) {
        let name = call.name().as_slice();
        if name == b"!" {
            if let Some(recv) = call.receiver() {
                return CondForm::Negated(recv);
            }
        } else if name == b"nil?" {
            if let Some(recv) = call.receiver() {
                return CondForm::NilCheck(recv);
            }
        }
    }
    CondForm::Bare(*pred)
}

/// The checked variable for a body that runs when the predicate is
/// *truthy* (a regular/modifier `if`, or a ternary's true-branch body).
fn if_checked_variable<'pr>(form: &CondForm<'pr>) -> Option<Node<'pr>> {
    match *form {
        CondForm::Bare(n) | CondForm::NotNilCheck(n) => Some(n),
        CondForm::NilCheck(_) | CondForm::Negated(_) => None,
    }
}

/// The checked variable for a body that runs when the predicate is *falsy*
/// (`unless`, or a ternary's false-branch body). A bare predicate is never
/// valid here (RuboCop's `use_var_only_in_unless_modifier?`): running the
/// body when a plain variable is falsy is the wrong direction.
fn unless_checked_variable<'pr>(form: &CondForm<'pr>) -> Option<Node<'pr>> {
    match *form {
        CondForm::NilCheck(n) | CondForm::Negated(n) => Some(n),
        CondForm::Bare(_) | CondForm::NotNilCheck(_) => None,
    }
}

fn if_case<'pr>(if_node: &IfNode<'pr>) -> Option<(Node<'pr>, Node<'pr>)> {
    let body = single_body(if_node.statements())?;
    let checked = if_checked_variable(&classify_predicate(&if_node.predicate()))?;
    Some((checked, body))
}

fn unless_case<'pr>(n: &UnlessNode<'pr>) -> Option<(Node<'pr>, Node<'pr>)> {
    let body = single_body(n.statements())?;
    let checked = unless_checked_variable(&classify_predicate(&n.predicate()))?;
    Some((checked, body))
}

/// `if_keyword_loc().is_none()` marks a ternary (shared with other cops in
/// this codebase, e.g. `Style/RedundantParentheses`).
fn ternary_case<'pr>(n: &IfNode<'pr>) -> Option<(Node<'pr>, Node<'pr>)> {
    let true_branch = single_body(n.statements())?;
    let else_node = n.subsequent()?.as_else_node()?;
    let false_branch = single_body(else_node.statements())?;
    let form = classify_predicate(&n.predicate());
    let true_nil = true_branch.as_nil_node().is_some();
    let false_nil = false_branch.as_nil_node().is_some();
    if false_nil && !true_nil {
        Some((if_checked_variable(&form)?, true_branch))
    } else if true_nil && !false_nil {
        Some((unless_checked_variable(&form)?, false_branch))
    } else {
        None
    }
}

/// An `IfNode` reached as its own parent's `elsif` continuation: Prism gives
/// an `elsif` its own `if_keyword_loc` text of `"elsif"`, so this needs no
/// parent lookup.
fn is_elsif(ctx: &Context<'_>, if_node: &IfNode<'_>) -> bool {
    if_node.if_keyword_loc().is_some_and(|l| ctx.text(l.span()) == b"elsif")
}

/// RuboCop's `Node#==` for matching a checked variable against a body
/// occurrence, ignoring the difference between a safe-navigation (`&.`) and
/// a dot method call (`Node#matching_call_nodes?`).
fn matching_nodes(ctx: &Context<'_>, a: &Node<'_>, b: &Node<'_>) -> bool {
    if same_position(a, b) {
        return true;
    }
    if a.kind() != b.kind() {
        return false;
    }
    match a.kind() {
        NodeKind::LocalVariableReadNode => {
            a.as_local_variable_read_node().expect("kind matched").name().as_slice()
                == b.as_local_variable_read_node().expect("kind matched").name().as_slice()
        }
        NodeKind::SelfNode => true,
        NodeKind::InstanceVariableReadNode => {
            a.as_instance_variable_read_node().expect("kind matched").name().as_slice()
                == b.as_instance_variable_read_node().expect("kind matched").name().as_slice()
        }
        NodeKind::ClassVariableReadNode => {
            a.as_class_variable_read_node().expect("kind matched").name().as_slice()
                == b.as_class_variable_read_node().expect("kind matched").name().as_slice()
        }
        NodeKind::GlobalVariableReadNode => {
            a.as_global_variable_read_node().expect("kind matched").name().as_slice()
                == b.as_global_variable_read_node().expect("kind matched").name().as_slice()
        }
        NodeKind::ConstantReadNode => {
            a.as_constant_read_node().expect("kind matched").name().as_slice()
                == b.as_constant_read_node().expect("kind matched").name().as_slice()
        }
        NodeKind::ConstantPathNode => {
            let (pa, pb) = (
                a.as_constant_path_node().expect("kind matched"),
                b.as_constant_path_node().expect("kind matched"),
            );
            let names_match = match (pa.name(), pb.name()) {
                (Some(na), Some(nb)) => na.as_slice() == nb.as_slice(),
                _ => false,
            };
            let parents_match = match (pa.parent(), pb.parent()) {
                (None, None) => true,
                (Some(pra), Some(prb)) => matching_nodes(ctx, &pra, &prb),
                _ => false,
            };
            names_match && parents_match
        }
        NodeKind::CallNode => {
            let (ca, cb) =
                (a.as_call_node().expect("kind matched"), b.as_call_node().expect("kind matched"));
            if ca.name().as_slice() != cb.name().as_slice() {
                return false;
            }
            let receivers_match = match (ca.receiver(), cb.receiver()) {
                (None, None) => true,
                (Some(ra), Some(rb)) => matching_nodes(ctx, &ra, &rb),
                _ => false,
            };
            receivers_match && call_args_equal(ctx, &ca, &cb)
        }
        // Every other kind (literals, `self`-less collections, ...): fall
        // back to source-text equality, matching upstream's structural
        // `Node#==` closely enough for the simple literal arguments a
        // checked variable's method chain carries in practice.
        _ => ctx.text(a.span()) == ctx.text(b.span()),
    }
}

fn call_args_equal(ctx: &Context<'_>, a: &CallNode<'_>, b: &CallNode<'_>) -> bool {
    match (a.arguments(), b.arguments()) {
        (None, None) => true,
        (Some(aa), Some(bb)) => {
            let (al, bl) = (aa.arguments(), bb.arguments());
            al.len() == bl.len()
                && al.iter().zip(bl.iter()).all(|(x, y)| matching_nodes(ctx, &x, &y))
        }
        _ => false,
    }
}

/// RuboCop's `find_matching_receiver_invocation`, collapsed to a plain
/// `CallNode::receiver()` walk: since a Prism call carrying a block is still
/// one `CallNode` (the block is an attached field, not a wrapping
/// ancestor), the generic `Node#receiver` matcher's `any_block` arm never
/// applies here.
fn find_matching_receiver<'pr>(
    ctx: &Context<'_>,
    method_chain: Node<'pr>,
    checked: &Node<'pr>,
) -> Option<Node<'pr>> {
    let call = method_chain.as_call_node()?;
    let receiver = call.receiver()?;
    if matching_nodes(ctx, &receiver, checked) {
        Some(receiver)
    } else {
        find_matching_receiver(ctx, receiver, checked)
    }
}

/// Whether `node` is whitequark's `:send`/`:csend` shape rather than its
/// separate `:block` node: a Prism `CallNode` only stands in for a
/// `call_type?`/`send_type?` ancestor in the unrestricted-climb matchers
/// below (`find_method_chain`, `chain_length`, `negated?`,
/// `unsafe_method_used?`, `dotless_operator_call?`) when it has *no*
/// attached block -- one that does is whitequark's `:block` node wrapping
/// it instead, which those matchers climb straight through uncounted.
fn ancestor_call<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    if call.block().is_some() {
        None
    } else {
        Some(call)
    }
}

/// RuboCop's `find_method_chain`.
fn find_method_chain<'pr>(map: &ParentMap<'pr>, node: Node<'pr>) -> Node<'pr> {
    match parent_of(map, &node) {
        Some(parent) if ancestor_call(&parent).is_some() => find_method_chain(map, parent),
        _ => node,
    }
}

/// RuboCop's `chain_length`: counts ancestor calls of `method` up to and
/// including `method_chain`, skipping non-call ancestors without stopping
/// the climb.
///
/// A Prism `CallNode` always stands for whitequark's inner `send` -- real
/// and `call_type?` whether or not it carries a trailing block -- so it is
/// always counted here. Only a block-less call's span can ever coincide
/// with `method_chain`'s own span, though: one *with* a block merges that
/// inner `send` with the separate outer `:block` node whitequark would
/// wrap it in, and if `method_chain` is this same span it stands for that
/// *outer* `:block` -- which `each_ancestor(:call)` never yields, so it can
/// never terminate the climb here.
fn chain_length(map: &ParentMap<'_>, method_chain: &Node<'_>, method: &Node<'_>) -> i64 {
    let mut total = 0i64;
    let mut current = *method;
    loop {
        let Some(parent) = parent_of(map, &current) else { return total };
        if let Some(call) = parent.as_call_node() {
            if call.block().is_none() && same_position(&parent, method_chain) {
                return total + 1;
            }
            total += 1;
        }
        current = parent;
    }
}

/// RuboCop's `negated?`: climbs through every immediately-enclosing call
/// (a chain of calls above `node`, e.g. `!foo.bar?`'s `!`) to see whether
/// the outermost one is `!`.
fn negated(map: &ParentMap<'_>, node: &Node<'_>) -> bool {
    if let Some(parent) = parent_of(map, node) {
        if ancestor_call(&parent).is_some() {
            return negated(map, &parent);
        }
    }
    node.as_call_node().is_some_and(|c| c.name().as_slice() == b"!")
}

/// RuboCop's `unsafe_method?`.
fn unsafe_method(map: &ParentMap<'_>, is_ternary: bool, node: &Node<'_>) -> bool {
    if negated(map, node) {
        return true;
    }
    if is_ternary {
        return false;
    }
    let Some(call) = node.as_call_node() else { return false };
    call.is_attribute_write() || (call.call_operator_loc().is_none() && !call.is_safe_navigation())
}

/// RuboCop's `unsafe_method_used?`. See [`chain_length`]'s doc for why a
/// call ancestor is always processed here regardless of an attached
/// block, but only a block-less one's span can ever terminate the climb
/// by matching `method_chain`.
fn unsafe_method_used(
    map: &ParentMap<'_>,
    is_ternary: bool,
    chain_enabled: bool,
    nil_methods: &HashSet<Vec<u8>>,
    method_chain: &Node<'_>,
    method: &Node<'_>,
) -> bool {
    if unsafe_method(map, is_ternary, method) {
        return true;
    }
    let mut current = *method;
    loop {
        let Some(parent) = parent_of(map, &current) else { return false };
        if let Some(call) = parent.as_call_node() {
            if !chain_enabled {
                return true;
            }
            if unsafe_method(map, is_ternary, &parent) {
                return true;
            }
            if nil_methods.contains(call.name().as_slice()) {
                return true;
            }
            if call.block().is_none() && same_position(&parent, method_chain) {
                return false;
            }
        }
        current = parent;
    }
}

fn is_empty_method(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|c| c.name().as_slice() == b"empty?")
}

fn is_dotless_operator_method(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|c| {
        c.call_operator_loc().is_none() && OPERATOR_METHODS.contains(&c.name().as_slice())
    })
}

/// RuboCop's `dotless_operator_call?`: `method_call` itself, or the
/// outermost call reached by climbing while every ancestor stays a call, is
/// a bracket/operator method ("Ignored `foo&.[](index)` due to unclear
/// readability benefit").
fn dotless_operator_call(map: &ParentMap<'_>, method_call: &Node<'_>) -> bool {
    if is_dotless_operator_method(method_call) {
        return true;
    }
    let mut current = *method_call;
    while let Some(parent) = parent_of(map, &current) {
        if ancestor_call(&parent).is_some() {
            current = parent;
        } else {
            break;
        }
    }
    is_dotless_operator_method(&current)
}

fn is_double_colon(ctx: &Context<'_>, call: &CallNode<'_>) -> bool {
    call.call_operator_loc().is_some_and(|l| ctx.text(l.span()) == b"::")
}

/// RuboCop's `relevant_comment_ranges`: the line-gaps between `node`'s
/// direct children (and before the first / after the last), so a comment
/// physically inside a child keeps belonging to that child.
fn relevant_comment_line_ranges(ctx: &Context<'_>, node: &Node<'_>) -> Vec<(u32, u32)> {
    let mut ranges = Vec::new();
    let mut begin_line = ctx.line_col(node.span().start).line;
    for_each_child(node, |child| {
        ranges.push((begin_line, ctx.line_col(child.span().start).line));
        begin_line = ctx.last_line(child.span());
    });
    ranges.push((begin_line, ctx.last_line(node.span())));
    ranges
}

/// RuboCop's `comments`: every comment on a line inside one of `node`'s
/// gaps, joined by newlines, ready to be reinserted before the surviving
/// method call.
fn preserved_comments(ctx: &Context<'_>, node: &Node<'_>) -> Vec<u8> {
    let ranges = relevant_comment_line_ranges(ctx, node);
    let mut out = Vec::new();
    for c in ctx.comments() {
        if ranges.iter().any(|&(begin, end)| c.line >= begin && c.line < end) {
            if !out.is_empty() {
                out.push(b'\n');
            }
            out.extend_from_slice(ctx.text(c.span));
        }
    }
    out
}

/// RuboCop's `add_safe_nav_to_all_methods_in_chain`: walks strict ancestors
/// of `start` up to and including `method_chain`, inserting `&` before each
/// hop's dot unless it is an operator method.
fn add_safe_nav_chain_edits(
    map: &ParentMap<'_>,
    start: &Node<'_>,
    method_chain: &Node<'_>,
    edits: &mut Vec<Edit>,
) {
    let mut current = *start;
    while let Some(parent) = parent_of(map, &current) {
        let Some(call) = parent.as_call_node() else { break };
        if call.is_safe_navigation() || OPERATOR_METHODS.contains(&call.name().as_slice()) {
            // Already safe-nav (whitequark's `csend`, never `send`) or an
            // operator method: upstream's `next if !ancestor.send_type? ||
            // ancestor.operator_method?` skips the insertion *and* the
            // `ancestor == method_chain` stop check for this hop.
            current = parent;
            continue;
        }
        if let Some(dot) = call.call_operator_loc() {
            edits.push(Edit::insert(dot.span().start, b"&".to_vec()));
        }
        let stop = same_position(&parent, method_chain);
        current = parent;
        if stop {
            break;
        }
    }
}

fn with_trailing_space(ctx: &Context<'_>, span: Span) -> Span {
    ctx.with_surrounding_space(span, Side::Right, true, false)
}

/// One item of the flattened, source-ordered `and`/`or`-clause list RuboCop's
/// `collect_and_clauses` builds before slicing it into pairs.
#[derive(Clone, Copy)]
enum Item<'pr> {
    Op(Span),
    Operand(Node<'pr>),
}

fn item_pos(item: &Item<'_>) -> u32 {
    match item {
        Item::Op(s) => s.start,
        Item::Operand(n) => n.span().start,
    }
}

/// RuboCop's `and_parts`.
fn and_parts<'pr>(and_node: &AndNode<'pr>, items: &mut Vec<Item<'pr>>) {
    items.push(Item::Op(and_node.operator_loc().span()));
    let rhs = and_node.right();
    if !and_inside_begin(&rhs) {
        items.push(Item::Operand(rhs));
    }
    let lhs = and_node.left();
    if lhs.as_and_node().is_none() && !and_inside_begin(&lhs) {
        items.push(Item::Operand(lhs));
    }
}

/// RuboCop's `and_node.each_ancestor(:block).any?`: whether `node` has a
/// block ancestor *anywhere* in the file -- not just one entered while
/// descending from the currently-processed top `and` node, since the whole
/// chain may itself sit inside an outer block.
fn has_block_ancestor(map: &ParentMap<'_>, node: &Node<'_>) -> bool {
    let mut current = *node;
    while let Some(parent) = parent_of(map, &current) {
        if parent.kind() == NodeKind::BlockNode {
            return true;
        }
        current = parent;
    }
    false
}

/// RuboCop's `node.each_descendant(:and).inject(...) { |nodes, and_node|
/// concat_nodes(nodes, and_node) }`: every `and` descendant not nested
/// inside a block contributes its own `and_parts`.
fn collect_nested_and_parts<'pr>(
    map: &ParentMap<'pr>,
    node: &Node<'pr>,
    items: &mut Vec<Item<'pr>>,
) {
    if let Some(and_node) = node.as_and_node() {
        if !has_block_ancestor(map, node) {
            and_parts(&and_node, items);
        }
    }
    for_each_child(node, |child| {
        collect_nested_and_parts(map, child, items);
    });
}

/// RuboCop's `collect_and_clauses`: flattens the top `and` node's own parts
/// plus every qualifying nested `and` descendant's parts, sorts by physical
/// source position, then slides a `(2, 2)` window across the result to pair
/// up consecutive operands (and the operator between the first pair).
fn collect_and_clauses<'pr>(
    map: &ParentMap<'pr>,
    top: &AndNode<'pr>,
) -> Vec<(Node<'pr>, Option<Span>, Node<'pr>)> {
    let mut items = Vec::new();
    and_parts(top, &mut items);
    let top_node = top.as_node();
    for_each_child(&top_node, |child| {
        collect_nested_and_parts(map, child, &mut items);
    });
    items.sort_by_key(item_pos);

    let chunks: Vec<&[Item<'pr>]> = items.chunks(2).collect();
    let mut clauses = Vec::new();
    for window in chunks.windows(2) {
        let (chunk_a, chunk_b) = (window[0], window[1]);
        let Item::Operand(lhs) = chunk_a[0] else { continue };
        let lhs_op = chunk_a.get(1).and_then(|it| match it {
            Item::Op(s) => Some(*s),
            Item::Operand(_) => None,
        });
        let Item::Operand(rhs) = chunk_b[0] else { continue };
        clauses.push((lhs, lhs_op, rhs));
    }
    clauses
}

/// Transforms usages of a method call safeguarded by a check for the existence of the object to safe navigation (`&.`). Autocorrection is unsafe as it assumes the object will be `nil` or truthy, but never `false`.
#[derive(Debug, Clone)]
pub struct SafeNavigation {
    /// `NilMethods#nil_methods`: `nil.methods + other_stdlib_methods +
    /// AllowedMethods`.
    nil_methods: HashSet<Vec<u8>>,
    /// `ConvertCodeThatCanStartToReturnNil`.
    convert_returning_nil: bool,
    /// `MaxChainLength`.
    max_chain_length: i64,
    /// `Lint/SafeNavigationChain`'s own `Enabled`, read as a peer option.
    chain_enabled: bool,
    /// Offense spans already reported in this file, mirroring RuboCop's
    /// `Base#add_offense` silently dropping duplicate locations -- `on_and`
    /// re-derives the same clause from more than one physical `and` node.
    reported: HashSet<Span>,
}

/// The shared inputs of RuboCop's `on_if` body (regular/modifier
/// `if`/`unless` and ternary shapes alike, once `checked`/`body` have been
/// extracted), bundled to keep [`SafeNavigation::emit`]'s argument count
/// down.
#[derive(Clone, Copy)]
struct CondCase<'pr> {
    node_span: Span,
    checked: Node<'pr>,
    body: Node<'pr>,
    is_ternary: bool,
    root: Node<'pr>,
}

impl SafeNavigation {
    /// RuboCop's shared body of `on_if` (covering the regular/modifier
    /// `if`/`unless` and ternary shapes alike, once `checked`/`body` have
    /// been extracted).
    fn emit<'pr>(&mut self, case: CondCase<'pr>, map: &ParentMap<'pr>, ctx: &mut Context<'_>) {
        let CondCase { node_span, checked, body, is_ternary, root } = case;
        let Some(receiver) = find_matching_receiver(ctx, body, &checked) else { return };
        let Some(method_call) = parent_of(map, &receiver) else { return };
        let Some(mc_call) = method_call.as_call_node() else { return };
        if chain_length(map, &body, &receiver) > self.max_chain_length {
            return;
        }
        if unsafe_method_used(
            map,
            is_ternary,
            self.chain_enabled,
            &self.nil_methods,
            &body,
            &method_call,
        ) {
            return;
        }
        if is_empty_method(&body) {
            return;
        }
        if dotless_operator_call(map, &method_call) || is_double_colon(ctx, &mc_call) {
            return;
        }

        let remove_before = Span::new(node_span.start, body.span().start);
        let remove_after = Span::new(body.span().end, node_span.end);
        let mut edits = vec![Edit::delete(remove_before), Edit::delete(remove_after)];

        if checked.as_call_node().is_some_and(|c| c.is_safe_navigation()) {
            edits.push(Edit::replace(receiver.span(), ctx.text(checked.span()).to_vec()));
        }
        if !mc_call.is_safe_navigation() {
            if let Some(dot) = mc_call.call_operator_loc() {
                edits.push(Edit::insert(dot.span().start, b"&".to_vec()));
            }
        }
        let comments = preserved_comments(ctx, &root);
        if !comments.is_empty() {
            let mut text = comments;
            text.push(b'\n');
            edits.push(Edit::insert(body.span().start, text));
        }
        add_safe_nav_chain_edits(map, &method_call, &body, &mut edits);

        ctx.report_with_fix(
            &Self::META,
            node_span,
            MSG,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }

    /// RuboCop's `on_and`.
    fn handle_and<'pr>(
        &mut self,
        and_node: &AndNode<'pr>,
        map: &ParentMap<'pr>,
        ctx: &mut Context<'_>,
    ) {
        let root = and_node.as_node();
        let is_rhs_or = and_with_rhs_or(and_node);
        let mut fixed = false;

        for (lhs, lhs_op, rhs) in collect_and_clauses(map, and_node) {
            let not_nil = as_not_nil_check(&lhs);
            let lhs_receiver = not_nil.unwrap_or(lhs);
            if !self.convert_returning_nil && not_nil.is_some() {
                continue;
            }
            let Some(rhs_receiver) = find_matching_receiver(ctx, strip_begin(rhs), &lhs_receiver)
            else {
                continue;
            };
            if chain_length(map, &rhs, &rhs_receiver) > self.max_chain_length {
                continue;
            }
            let Some(method) = parent_of(map, &rhs_receiver) else { continue };
            if unsafe_method_used(map, false, self.chain_enabled, &self.nil_methods, &rhs, &method)
            {
                continue;
            }
            if is_empty_method(&rhs) {
                continue;
            }
            let lhs_method_chain = find_method_chain(map, lhs_receiver);
            if !(same_position(&lhs_method_chain, &lhs_receiver) || not_nil.is_some()) {
                continue;
            }

            let offense_span = Span::new(lhs.span().start, rhs.span().end);
            if !self.reported.insert(offense_span) {
                continue;
            }

            if fixed || is_rhs_or {
                ctx.report(&Self::META, offense_span, MSG);
                continue;
            }

            let mut edits =
                vec![Edit::replace(rhs_receiver.span(), ctx.text(lhs_receiver.span()).to_vec())];
            if let Some(op_span) = lhs_op {
                edits.push(Edit::delete(with_trailing_space(ctx, lhs.span())));
                edits.push(Edit::delete(with_trailing_space(ctx, op_span)));
            }
            let comments = preserved_comments(ctx, &root);
            if !comments.is_empty() {
                let mut text = comments;
                text.push(b'\n');
                edits.push(Edit::insert(rhs.span().start, text));
            }
            add_safe_nav_chain_edits(map, &rhs_receiver, &rhs, &mut edits);

            ctx.report_with_fix(
                &Self::META,
                offense_span,
                MSG,
                Fix { applicability: Applicability::Unsafe, edits },
            );
            fixed = true;
        }
    }

    /// Drives the traversal ourselves (`kinds: &[]`): builds one whole-file
    /// parent map up front so [`unsafe_method_used`]/[`chain_length`]/
    /// [`negated`] can see ancestors *above* the processed `if`/`unless`/
    /// `and` node too (e.g. an enclosing bare method call whose receiver-less
    /// send counts as "unsafe" per `unsafe_method?`), then walks every node.
    fn walk<'pr>(&mut self, node: &Node<'pr>, map: &ParentMap<'pr>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::IfNode => {
                let if_node = node.as_if_node().expect("kind matched");
                if if_node.if_keyword_loc().is_none() {
                    if let Some((checked, body)) = ternary_case(&if_node) {
                        self.emit(
                            CondCase {
                                node_span: if_node.as_node().span(),
                                checked,
                                body,
                                is_ternary: true,
                                root: if_node.as_node(),
                            },
                            map,
                            ctx,
                        );
                    }
                } else if !is_elsif(ctx, &if_node) && if_node.subsequent().is_none() {
                    if let Some((checked, body)) = if_case(&if_node) {
                        self.emit(
                            CondCase {
                                node_span: if_node.as_node().span(),
                                checked,
                                body,
                                is_ternary: false,
                                root: if_node.as_node(),
                            },
                            map,
                            ctx,
                        );
                    }
                }
            }
            NodeKind::UnlessNode => {
                let n = node.as_unless_node().expect("kind matched");
                if n.else_clause().is_none() {
                    if let Some((checked, body)) = unless_case(&n) {
                        self.emit(
                            CondCase {
                                node_span: n.as_node().span(),
                                checked,
                                body,
                                is_ternary: false,
                                root: n.as_node(),
                            },
                            map,
                            ctx,
                        );
                    }
                }
            }
            NodeKind::AndNode => {
                let n = node.as_and_node().expect("kind matched");
                self.handle_and(&n, map, ctx);
            }
            _ => {}
        }
        for_each_child(node, |child| self.walk(child, map, ctx));
    }
}

impl Rule for SafeNavigation {
    const META: RuleMeta = RuleMeta {
        name: "Style/SafeNavigation",
        department: Department::Style,
        summary: "Transforms usages of a method call safeguarded by a check for the existence of the object to safe navigation (`&.`). Autocorrection is unsafe as it assumes the object will be `nil` or truthy, but never `false`.",
        explanation: "Transforms usages of a method call safeguarded by a non `nil` check for the \
variable whose method is being called to safe navigation (`&.`). If there is a method chain, all \
of the methods in the chain need to be checked for safety, and all of the methods will need to be \
changed to use safe navigation.

# Examples

```ruby
# bad
foo.bar if foo
foo.bar.baz if foo
foo.bar(param1, param2) if foo
foo.bar { |e| e.something } if foo
foo.bar(param) { |e| e.something } if foo

foo.bar if !foo.nil?
foo.bar unless !foo
foo.bar unless foo.nil?

foo && foo.bar
foo && foo.bar.baz
foo && foo.bar(param1, param2)
foo && foo.bar { |e| e.something }
foo && foo.bar(param) { |e| e.something }

foo ? foo.bar : nil
foo.nil? ? nil : foo.bar
!foo.nil? ? foo.bar : nil
!foo ? nil : foo.bar

# good
foo&.bar
foo&.bar&.baz
foo&.bar(param1, param2)
foo&.bar { |e| e.something }
foo&.bar(param) { |e| e.something }
foo && foo.bar.baz.qux # method chain with more than 2 methods
foo && foo.nil? # method that `nil` responds to
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[
            ConfigOption {
                name: "ConvertCodeThatCanStartToReturnNil",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Enables conversion of code such as `!foo.nil? && foo.bar` to `foo&.bar`, \
which as a whole can start returning `nil` in addition to what the method itself returns.",
            },
            ConfigOption {
                name: "MaxChainLength",
                default: ConfigDefault::Int(2),
                allowed: &[],
                doc: "Maximum length of method chains for register an offense.",
            },
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(&["present?", "blank?", "presence", "try", "try!"]),
                allowed: &[],
                doc: "Methods that `nil` may safely respond to for the purpose of an `&&`-chained \
method call, beyond `nil`'s own instance methods.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let mut nil_methods: HashSet<Vec<u8>> = NIL_METHODS.iter().map(|m| m.to_vec()).collect();
        for allowed in options.str_list("AllowedMethods") {
            nil_methods.insert(allowed.into_bytes());
        }
        let convert_returning_nil = options.bool("ConvertCodeThatCanStartToReturnNil");
        let max_chain_length = options.int("MaxChainLength");
        let chain_enabled = options
            .peer("Lint/SafeNavigationChain", "Enabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(true);
        Ok(Self {
            nil_methods,
            convert_returning_nil,
            max_chain_length,
            chain_enabled,
            reported: HashSet::new(),
        })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let map = build_parent_map(&root);
        self.walk(&root, &map, ctx);
    }
}
