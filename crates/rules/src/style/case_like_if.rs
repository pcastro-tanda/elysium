//! `Style/CaseLikeIf`, ported from RuboCop's
//! `lib/rubocop/cop/style/case_like_if.rb` plus the `MinBranchesCount` mixin
//! it includes.
//!
//! # Node shapes
//!
//! whitequark's single `:if` node type covers a ternary, a modifier-form
//! `if`, a plain `if`, and (recursively, via its own `else_branch`) an
//! `elsif` link; Prism keeps the same unification but spells the four cases
//! differently: a ternary has no [`IfNode::if_keyword_loc`] at all, a
//! modifier form has no [`IfNode::end_keyword_loc`], and an `elsif` link is
//! reached through [`IfNode::subsequent`] (`Some(Node::IfNode { .. })`,
//! whose own `if_keyword_loc` reads `"elsif"`) rather than being the final
//! `else` (`Some(Node::ElseNode { .. })`) or nothing (`None`). Because the
//! generic tree walk reaches every `elsif` link's `IfNode` as an ordinary
//! descendant of the top `if`, [`should_check`]'s `keyword == "elsif"` guard
//! stops each of those from being treated as its own top-level `if`, exactly
//! as upstream's `elsif?` does.
//!
//! [`MinBranchesCount::if_conditional_branches`] and the cop's own
//! `branch_conditions` walk the same `subsequent` chain but stop at
//! different points: the former only ever follows another `elsif` link (so
//! a trailing plain `else` is never counted as a branch), while the latter
//! also folds in that trailing `else`'s own predicate slot -- except there
//! is none, so in practice both stop at the same node; the difference only
//! matters because `if_conditional_branches` is checked once up front
//! ([`should_check`]) while `branch_conditions`'s equivalent
//! ([`Rule::enter`]'s own loop) is walked to collect each branch's `when`
//! conditions.
//!
//! # Target and condition extraction
//!
//! [`find_target`] locates the common expression (upstream's `target`, e.g.
//! `status` in `if status == :active`) from the first branch's predicate
//! only; [`collect_conditions`] then re-derives each branch's own `when`
//! value(s) against that fixed target, recursing through `||` and
//! parenthesized sub-expressions. Both dispatch on the same handful of
//! shapes: `is_a?`, `==`/`eql?`/`equal?`, `===`, `include?`/`cover?`, and
//! `match`/`match?`/`=~` -- plus, only for the *target* search, a bare
//! `MatchWriteNode` (Prism's node for a literal regexp with named captures
//! on the left of `=~`, whitequark's `match_with_lvasgn`; its wrapped `call`
//! field already has the right receiver/argument shape, so no separate
//! target-search or condition-collection logic is needed beyond unwrapping
//! it first).
//!
//! whitequark's `begin` node (an explicitly parenthesized group) is Prism's
//! [`ruby_ast::NodeKind::ParenthesesNode`]; [`first_child_of_parens`]
//! mirrors upstream's `node.children.first` (used by both [`find_target`]
//! and [`collect_conditions`]'s own `:begin` arm), while [`deparenthesize`]
//! mirrors the general-purpose `node.children.last` loop upstream applies
//! to the two operands of an equality/match comparison before comparing
//! them against `target` or returning them as a `when` value.
//!
//! `Node#==` (comparing an extracted operand against `target`) is
//! approximated by source-text equality, the same approximation
//! `Style::CombinableLoops` and `Lint::SelfAssignment` use for the same
//! problem.
//!
//! # Named captures
//!
//! `regexp_with_working_captures?` guards against rewriting a branch whose
//! `=~`/`match` (never `match?`, which never assigns locals) has a literal
//! regexp operand with named captures: converting to `case`/`when` would
//! drop the implicit local-variable assignments Ruby makes from those
//! captures. [`has_named_capture`] is a small purpose-built scanner over a
//! [`ruby_ast::NodeKind::RegularExpressionNode`]'s content bytes (adapted
//! from `Lint::MixedRegexpCaptureTypes`'s `scan_captures`, restricted to the
//! named case), matching upstream's `each_capture(named: true).any?`.
//! [`ruby_ast::NodeKind::InterpolatedRegularExpressionNode`] is treated as
//! never having named captures (documented in `blind_spots`): its content is
//! only known at runtime.
//!
//! # Autocorrection
//!
//! One [`Fix`] inserts `"case #{target}\n#{indent}"` immediately before the
//! top `if` (RuboCop's `corrector.insert_before`) and replaces each branch's
//! own `if`/`elsif` keyword through its predicate's end (RuboCop's
//! `correction_range`) with `"when #{conditions}"`. The insertion is a
//! zero-length edit at the same offset the first branch's replacement
//! starts at; [`crate::Rule`]'s fix applicator treats an insertion and an
//! adjacent (non-enclosing) replacement as non-conflicting and orders them
//! by span end, so the two combine correctly without special-casing here.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, IfNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Convert `if-elsif` to `case-when`.";

/// Identifies places where `if-elsif` constructions can be replaced with `case-when`.
#[derive(Debug, Clone)]
pub struct CaseLikeIf {
    /// RuboCop's `MinBranchesCount`: the number of `if`/`elsif` branches
    /// (never counting a trailing plain `else`) an `if` chain needs before
    /// this cop considers it.
    min_branches_count: i64,
}

impl Rule for CaseLikeIf {
    const META: RuleMeta = RuleMeta {
        name: "Style/CaseLikeIf",
        department: Department::Style,
        summary:
            "Identifies places where `if-elsif` constructions can be replaced with `case-when`.",
        explanation: "\
```ruby
# MinBranchesCount: 3 (default)
# bad
if status == :active
  perform_action
elsif status == :inactive || status == :hibernating
  check_timeout
elsif status == :invalid
  report_invalid
else
  final_action
end

# good
case status
when :active
  perform_action
when :inactive, :hibernating
  check_timeout
when :invalid
  report_invalid
else
  final_action
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode],
        config: &[linter::ConfigOption {
            name: "MinBranchesCount",
            default: linter::ConfigDefault::Int(3),
            allowed: &[],
            doc: "The number of branches `if` needs to have to trigger this cop.",
        }],
        blind_spots: "A `match`/`match?`/`=~` operand that is an \
            `InterpolatedRegularExpressionNode` (a regexp literal with `#{...}` \
            interpolation) is treated as never having named captures, since its \
            content is only known at runtime; RuboCop's `regexp_parser`-based check \
            has the same limitation for a dynamic pattern.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { min_branches_count: options.int("MinBranchesCount") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let if_node = node.as_if_node().expect("kind matched");
        if !self.should_check(ctx, &if_node) {
            return;
        }
        let Some(target) = find_target(if_node.predicate()) else { return };

        let mut branches: Vec<(Span, Vec<Node<'_>>)> = Vec::new();
        let mut current = Some(if_node);
        while let Some(cur) = current {
            let branch_condition = cur.predicate();
            if regexp_with_working_captures(ctx, branch_condition) {
                // Upstream's `return false` from inside the `each` block:
                // a named-capture `=~`/`match` anywhere in the chain aborts
                // the whole `if`, not just this branch.
                return;
            }
            let mut conditions = Vec::new();
            if !collect_conditions(ctx, branch_condition, target, &mut conditions) {
                return;
            }
            let keyword_span = cur.if_keyword_loc().expect("if/elsif always has a keyword").span();
            let range = Span::new(keyword_span.start, branch_condition.span().end);
            branches.push((range, conditions));
            current = cur.subsequent().and_then(|s| s.as_if_node());
        }

        let column = ctx.line_col(node.span().start).column;
        let indent = " ".repeat(column as usize);
        let target_source = String::from_utf8_lossy(ctx.text(target.span()));
        let mut edits = Vec::with_capacity(branches.len() + 1);
        edits.push(Edit::insert(
            node.span().start,
            format!("case {target_source}\n{indent}").into_bytes(),
        ));
        for (range, conditions) in &branches {
            let joined = conditions
                .iter()
                .map(|c| String::from_utf8_lossy(ctx.text(c.span())).into_owned())
                .collect::<Vec<_>>()
                .join(", ");
            edits.push(Edit::replace(*range, format!("when {joined}").into_bytes()));
        }
        let fix = Fix { applicability: Applicability::Unsafe, edits };
        ctx.report_with_fix(&Self::META, node.span(), MSG, fix);
    }
}

impl CaseLikeIf {
    /// RuboCop's `should_check?`: not a ternary, not itself an `elsif` link,
    /// not modifier-form, has at least one `elsif` link, and the chain (not
    /// counting a trailing plain `else`) has at least `MinBranchesCount`
    /// branches.
    fn should_check(&self, ctx: &Context<'_>, if_node: &IfNode<'_>) -> bool {
        let Some(keyword_loc) = if_node.if_keyword_loc() else { return false }; // ternary
        if ctx.text(keyword_loc.span()) == b"elsif" {
            return false;
        }
        if if_node.end_keyword_loc().is_none() {
            return false; // modifier form
        }
        if if_node.subsequent().is_none_or(|s| s.as_if_node().is_none()) {
            return false; // no `elsif` link at all (plain `else` or nothing)
        }
        branch_count(if_node) >= self.min_branches_count
    }
}

/// `MinBranchesCount#if_conditional_branches`'s `.size`: the number of
/// `if`/`elsif` links in the chain, stopping before a trailing plain `else`.
fn branch_count(if_node: &IfNode<'_>) -> i64 {
    let mut count = 0i64;
    let mut current = Some(*if_node);
    while let Some(cur) = current {
        count += 1;
        current = cur.subsequent().and_then(|s| s.as_if_node());
    }
    count
}

/// RuboCop's `find_target`.
fn find_target(node: Node<'_>) -> Option<Node<'_>> {
    match node.kind() {
        NodeKind::ParenthesesNode => {
            let paren = node.as_parentheses_node().expect("kind matched");
            find_target(first_child_of_parens(&paren)?)
        }
        NodeKind::OrNode => {
            let or_node = node.as_or_node().expect("kind matched");
            find_target(or_node.left())
        }
        NodeKind::MatchWriteNode => {
            let mw = node.as_match_write_node().expect("kind matched");
            let call = mw.call();
            let lhs = call.receiver()?;
            let rhs = first_argument(&call);
            if is_regexp(&lhs) {
                rhs
            } else if rhs.is_some_and(|r| is_regexp(&r)) {
                Some(lhs)
            } else {
                None
            }
        }
        NodeKind::CallNode => {
            let call = node.as_call_node().expect("kind matched");
            find_target_in_send_node(&call)
        }
        _ => None,
    }
}

/// RuboCop's `find_target_in_send_node`.
fn find_target_in_send_node<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    if call.is_safe_navigation() {
        return None;
    }
    match call.name().as_slice() {
        b"is_a?" => call.receiver(),
        b"==" | b"eql?" | b"equal?" => find_target_in_equality_node(call),
        b"===" => first_argument(call),
        b"include?" | b"cover?" => find_target_in_include_or_cover_node(call),
        b"match" | b"match?" | b"=~" => find_target_in_match_node(call),
        _ => None,
    }
}

/// RuboCop's `find_target_in_equality_node`.
fn find_target_in_equality_node<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let argument = first_argument(call)?;
    let receiver = call.receiver()?;
    if is_literal(&argument) || is_const_reference(&argument) {
        Some(receiver)
    } else if is_literal(&receiver) || is_const_reference(&receiver) {
        Some(argument)
    } else {
        None
    }
}

/// RuboCop's `find_target_in_include_or_cover_node`.
fn find_target_in_include_or_cover_node<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let receiver = call.receiver()?;
    if is_range(&deparenthesize(receiver)) {
        first_argument(call)
    } else {
        None
    }
}

/// RuboCop's `find_target_in_match_node`.
fn find_target_in_match_node<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let receiver = call.receiver()?;
    let argument = first_argument(call);
    if is_regexp(&receiver) {
        argument
    } else if argument.is_some_and(|a| is_regexp(&a)) {
        Some(receiver)
    } else {
        None
    }
}

/// RuboCop's `collect_conditions`, returning its trailing `conditions <<
/// condition if condition` truthiness (`false` once any leaf fails to
/// produce a condition, short-circuiting `||` the same way).
fn collect_conditions<'pr>(
    ctx: &Context<'_>,
    node: Node<'pr>,
    target: Node<'pr>,
    conditions: &mut Vec<Node<'pr>>,
) -> bool {
    match node.kind() {
        NodeKind::ParenthesesNode => {
            let paren = node.as_parentheses_node().expect("kind matched");
            match first_child_of_parens(&paren) {
                Some(child) => collect_conditions(ctx, child, target, conditions),
                None => false,
            }
        }
        NodeKind::OrNode => {
            let or_node = node.as_or_node().expect("kind matched");
            collect_conditions(ctx, or_node.left(), target, conditions)
                && collect_conditions(ctx, or_node.right(), target, conditions)
        }
        NodeKind::MatchWriteNode => {
            let mw = node.as_match_write_node().expect("kind matched");
            let call = mw.call();
            let condition = call
                .receiver()
                .zip(first_argument(&call))
                .and_then(|(lhs, rhs)| condition_from_binary_op(ctx, lhs, rhs, target));
            push_condition(conditions, condition)
        }
        NodeKind::CallNode => {
            let call = node.as_call_node().expect("kind matched");
            let condition = condition_from_send_node(ctx, &call, target);
            push_condition(conditions, condition)
        }
        _ => false,
    }
}

/// Appends `condition` to `conditions` and returns whether it was present,
/// mirroring the truthiness of upstream's `conditions << condition if
/// condition`.
fn push_condition<'pr>(conditions: &mut Vec<Node<'pr>>, condition: Option<Node<'pr>>) -> bool {
    match condition {
        Some(c) => {
            conditions.push(c);
            true
        }
        None => false,
    }
}

/// RuboCop's `condition_from_send_node`.
fn condition_from_send_node<'pr>(
    ctx: &Context<'_>,
    call: &CallNode<'pr>,
    target: Node<'pr>,
) -> Option<Node<'pr>> {
    if call.is_safe_navigation() {
        return None;
    }
    match call.name().as_slice() {
        b"is_a?" => {
            if same_node(ctx, call.receiver(), Some(target)) {
                first_argument(call)
            } else {
                None
            }
        }
        b"==" | b"eql?" | b"equal?" => condition_from_equality_node(ctx, call, target),
        b"=~" | b"match" | b"match?" => condition_from_match_node(ctx, call, target),
        b"===" => {
            if same_node(ctx, first_argument(call), Some(target)) {
                call.receiver()
            } else {
                None
            }
        }
        b"include?" | b"cover?" => condition_from_include_or_cover_node(ctx, call, target),
        _ => None,
    }
}

/// RuboCop's `condition_from_equality_node`.
fn condition_from_equality_node<'pr>(
    ctx: &Context<'_>,
    call: &CallNode<'pr>,
    target: Node<'pr>,
) -> Option<Node<'pr>> {
    let receiver = call.receiver()?;
    let argument = first_argument(call)?;
    let condition = condition_from_binary_op(ctx, receiver, argument, target)?;
    (!is_class_reference(&condition)).then_some(condition)
}

/// RuboCop's `condition_from_match_node`.
fn condition_from_match_node<'pr>(
    ctx: &Context<'_>,
    call: &CallNode<'pr>,
    target: Node<'pr>,
) -> Option<Node<'pr>> {
    let receiver = call.receiver()?;
    let argument = first_argument(call)?;
    condition_from_binary_op(ctx, receiver, argument, target)
}

/// RuboCop's `condition_from_include_or_cover_node`.
fn condition_from_include_or_cover_node<'pr>(
    ctx: &Context<'_>,
    call: &CallNode<'pr>,
    target: Node<'pr>,
) -> Option<Node<'pr>> {
    let receiver = deparenthesize(call.receiver()?);
    if !is_range(&receiver) {
        return None;
    }
    let argument = first_argument(call)?;
    same_node(ctx, Some(argument), Some(target)).then_some(receiver)
}

/// RuboCop's `condition_from_binary_op`.
fn condition_from_binary_op<'pr>(
    ctx: &Context<'_>,
    lhs: Node<'pr>,
    rhs: Node<'pr>,
    target: Node<'pr>,
) -> Option<Node<'pr>> {
    let lhs = deparenthesize(lhs);
    let rhs = deparenthesize(rhs);
    if same_node(ctx, Some(lhs), Some(target)) {
        Some(rhs)
    } else if same_node(ctx, Some(rhs), Some(target)) {
        Some(lhs)
    } else {
        None
    }
}

/// RuboCop's generic `Node#==`, approximated by source-text equality (see
/// the module doc).
fn same_node(ctx: &Context<'_>, a: Option<Node<'_>>, b: Option<Node<'_>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => ctx.text(a.span()) == ctx.text(b.span()),
        (None, None) => true,
        _ => false,
    }
}

/// RuboCop's `call.first_argument`.
fn first_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    call.arguments().and_then(|args| args.arguments().first())
}

/// RuboCop's `deparenthesize`: unwraps a chain of explicit parentheses down
/// to the last statement of the innermost one, or `node` itself if it is
/// never a `ParenthesesNode` to begin with.
fn deparenthesize(mut node: Node<'_>) -> Node<'_> {
    while let Some(paren) = node.as_parentheses_node() {
        let Some(body) = paren.body() else { break };
        node = match body.as_statements_node() {
            Some(stmts) => match stmts.body().last() {
                Some(last) => last,
                None => break,
            },
            None => body,
        };
    }
    node
}

/// The single-recursion counterpart to [`deparenthesize`] used by
/// [`find_target`]/[`collect_conditions`]'s own `:begin` arms (upstream's
/// `node.children.first`, rather than `deparenthesize`'s `.last` loop).
fn first_child_of_parens<'pr>(paren: &ruby_ast::node::ParenthesesNode<'pr>) -> Option<Node<'pr>> {
    let body = paren.body()?;
    match body.as_statements_node() {
        Some(stmts) => stmts.body().first(),
        None => Some(body),
    }
}

/// RuboCop's `regexp_type?`: a regexp literal, with or without
/// interpolation.
fn is_regexp(node: &Node<'_>) -> bool {
    node.as_regular_expression_node().is_some()
        || node.as_interpolated_regular_expression_node().is_some()
}

/// `rubocop-ast`'s `Node#range_type?`.
fn is_range(node: &Node<'_>) -> bool {
    node.as_range_node().is_some()
}

/// `rubocop-ast`'s `Node#literal?` (the `LITERALS` node-kind list).
fn is_literal(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::ArrayNode
            | NodeKind::HashNode
            | NodeKind::RegularExpressionNode
            | NodeKind::InterpolatedRegularExpressionNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::RangeNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
    )
}

/// The constant's own (rightmost) segment, for both a bare
/// `ConstantReadNode` and a namespaced `ConstantPathNode` (whitequark's
/// single `:const` node type).
fn short_const_name<'pr>(node: &Node<'pr>) -> Option<&'pr [u8]> {
    if let Some(read) = node.as_constant_read_node() {
        Some(read.name().as_slice())
    } else {
        node.as_constant_path_node().and_then(|path| path.name().map(|n| n.as_slice()))
    }
}

/// RuboCop's `const_reference?`: a constant whose own segment reads as
/// `SCREAMING_SNAKE_CASE` -- more than one character, and containing no
/// lowercase letter (case-folding to uppercase would change nothing).
fn is_const_reference(node: &Node<'_>) -> bool {
    short_const_name(node)
        .is_some_and(|name| name.len() > 1 && !name.iter().any(u8::is_ascii_lowercase))
}

/// RuboCop's `class_reference?`: a constant whose own segment contains a
/// lowercase letter, i.e. looks like a class/module name rather than a
/// `SCREAMING_SNAKE_CASE` constant.
fn is_class_reference(node: &Node<'_>) -> bool {
    short_const_name(node).is_some_and(|name| name.iter().any(u8::is_ascii_lowercase))
}

/// RuboCop's `regexp_with_working_captures?`: `=~` (never `match?`, which
/// assigns no locals) or `match` against a literal regexp operand with
/// named captures, which autocorrection would silently break by dropping
/// their implicit local-variable assignment.
fn regexp_with_working_captures(ctx: &Context<'_>, node: Node<'_>) -> bool {
    match node.kind() {
        NodeKind::MatchWriteNode => {
            let mw = node.as_match_write_node().expect("kind matched");
            let call = mw.call();
            call.message_loc().is_some_and(|l| ctx.text(l.span()) == b"=~")
                && call.receiver().is_some_and(|r| has_named_capture(ctx, &r))
        }
        NodeKind::CallNode => {
            let call = node.as_call_node().expect("kind matched");
            if call.name().as_slice() != b"match" {
                return false;
            }
            call.receiver().is_some_and(|r| has_named_capture(ctx, &r))
                || first_argument(&call).is_some_and(|a| has_named_capture(ctx, &a))
        }
        _ => false,
    }
}

/// RuboCop's `regexp_with_named_captures?`, restricted to a plain (never
/// interpolated -- see the module doc and `blind_spots`)
/// `RegularExpressionNode`.
fn has_named_capture(ctx: &Context<'_>, node: &Node<'_>) -> bool {
    let Some(regexp) = node.as_regular_expression_node() else { return false };
    scan_for_named_capture(ctx.text(regexp.content_loc().span()))
}

/// How an unescaped, non-class `(` at some position classifies, and how
/// many bytes of the group's opening syntax to skip past. Adapted from
/// `Lint::MixedRegexpCaptureTypes`'s `classify_group`, restricted to
/// whether the group is named.
enum GroupKind {
    Named,
    Other,
}

/// Classifies the group opened by `buf[pos] == '('`. Returns its kind and
/// the number of bytes (starting at `pos`) making up its opening syntax.
/// Adapted from `Lint::MixedRegexpCaptureTypes`'s `classify_group`.
fn classify_group(buf: &[u8], pos: usize, len: usize) -> (GroupKind, usize) {
    if pos + 1 >= len || buf[pos + 1] != b'?' {
        return (GroupKind::Other, 1);
    }
    if pos + 2 >= len {
        return (GroupKind::Other, 2);
    }
    match buf[pos + 2] {
        b'<' => {
            if pos + 3 < len && matches!(buf[pos + 3], b'=' | b'!') {
                (GroupKind::Other, 4)
            } else {
                (GroupKind::Named, 3)
            }
        }
        b'\'' => (GroupKind::Named, 3),
        b'#' => {
            let mut i = pos + 3;
            while i < len && buf[i] != b')' {
                i += 1;
            }
            let end = if i < len { i + 1 } else { i };
            (GroupKind::Other, end - pos)
        }
        _ => (GroupKind::Other, 2),
    }
}

/// Scans `buf` (a regexp literal's content bytes) for a top-level named
/// capture group, tracking character-class depth so a `(` inside `[...]`
/// is never mistaken for a group. Adapted from
/// `Lint::MixedRegexpCaptureTypes`'s `scan_captures`, stopping as soon as a
/// named capture is found instead of also counting numbered ones.
fn scan_for_named_capture(buf: &[u8]) -> bool {
    let len = buf.len();
    let mut pos = 0usize;
    let mut depth: u32 = 0;
    while pos < len {
        let c = buf[pos];
        if c == b'\\' {
            pos += 1;
            if pos >= len {
                break;
            }
            pos += utf8_len(buf, pos);
            continue;
        }
        if c == b'[' {
            depth += 1;
            pos += 1;
            continue;
        }
        if c == b']' && depth > 0 {
            depth -= 1;
            pos += 1;
            continue;
        }
        if depth == 0 && c == b'(' {
            let (kind, advance) = classify_group(buf, pos, len);
            if matches!(kind, GroupKind::Named) {
                return true;
            }
            pos += advance.max(1);
            continue;
        }
        pos += utf8_len(buf, pos);
    }
    false
}

/// The byte length of the UTF-8 sequence starting at `buf[pos]`, clamped to
/// the buffer's remaining length. Copied from
/// `Lint::MixedRegexpCaptureTypes`.
fn utf8_len(buf: &[u8], pos: usize) -> usize {
    if pos >= buf.len() {
        return 1;
    }
    let b = buf[pos];
    let want = if b & 0x80 == 0 {
        1
    } else if b & 0xE0 == 0xC0 {
        2
    } else if b & 0xF0 == 0xE0 {
        3
    } else if b & 0xF8 == 0xF0 {
        4
    } else {
        1
    };
    want.min(buf.len() - pos)
}
