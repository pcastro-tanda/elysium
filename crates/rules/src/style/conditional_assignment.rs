//! `Style/ConditionalAssignment`, ported from RuboCop's
//! `lib/rubocop/cop/style/conditional_assignment.rb`.
//!
//! Whitequark unifies every writable target (`lvasgn`, `ivasgn`, `cvasgn`,
//! `gvasgn`, `casgn`, `masgn`) and every compound-assignment wrapper
//! (`op_asgn`/`and_asgn`/`or_asgn`) plus attribute/index writers (`send`)
//! under a handful of node shapes whose common thread is "has an
//! `expression`/value". Prism instead gives each target/operator
//! combination its own node kind (`LocalVariableWriteNode`,
//! `LocalVariableOperatorWriteNode`, `IndexAndWriteNode`,
//! `CallOrWriteNode`, ...); [`rhs_of`] dispatches all of them (plus plain
//! attribute/index/comparison writers, which Prism keeps as an ordinary
//! [`ruby_ast::node::CallNode`], matching upstream's `assignment_type?`
//! send pattern) to their `value()`/last-argument uniformly, and [`lhs`]
//! reconstructs upstream's `lhs`/`lhs_for_send`/`lhs_for_casgn` the same
//! way for the `assign_to_condition` corrector.
//!
//! `EnforcedStyle: assign_inside_condition`'s corrector never needs to
//! reconstruct the assignment prefix: it just relocates the *original* raw
//! text between the target and its condition
//! (`node.source_range.begin.join(condition.source_range.begin)`) into each
//! branch. The reindentation is upstream's `remove_whitespace_in_branches`
//! node-by-node walk ([`collect_branch_whitespace`]): every node in the
//! branch, itself included, has the run of `node.column - column - 2` bytes
//! before it removed when that run is pure whitespace. Prism's extra
//! wrapper nodes coincide with a whitequark node's start position (a
//! `StatementsNode` with its first statement, an `ArgumentsNode` with its
//! first argument), so they only ever produce a duplicate removal -- except
//! [`NodeKind::ElseNode`]/[`NodeKind::EnsureNode`], which start at a
//! keyword whitequark has no node for and are therefore skipped.
//!
//! `EnforcedStyle: assign_to_condition` (RuboCop's actual default) instead
//! leaves every branch's own indentation untouched: its corrector
//! (`IfCorrector`/`CaseCorrector`/`TernaryCorrector`) only ever inserts the
//! reconstructed `lhs` before the conditional and replaces each branch's
//! tail statement with its own RHS source in place, plus -- when
//! `Layout/EndAlignment`'s `EnforcedStyleAlignWith` is `keyword` (the real
//! default) -- pads before the terminal `end` with `lhs.length` spaces
//! ([`build_outer_correction`]).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use linter::{ConfigDefault, ConfigOption};
use ruby_ast::node::{
    ArgumentsNode, CallNode, CaseMatchNode, CaseNode, ElseNode, IfNode, StatementsNode, UnlessNode,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind, NodeList, Parsed};
use ruby_source::{SourceFile, Span};

/// RuboCop's `MSG` (the `assign_to_condition` style).
const MSG: &str = "Use the return of the conditional for variable assignment and comparison.";
/// RuboCop's `ASSIGN_TO_CONDITION_MSG` (the `assign_inside_condition` style
/// -- despite the constant's name, this is the message *that* style uses).
const ASSIGN_TO_CONDITION_MSG: &str = "Assign variables inside of conditionals.";

/// RuboCop's `ConfigurableEnforcedStyle` values for this cop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    AssignToCondition,
    AssignInsideCondition,
}

/// Use the return value of `if` and `case` statements for assignment to a variable and variable comparison instead of assigning that variable inside of each branch.
#[derive(Debug, Clone)]
pub struct ConditionalAssignment {
    style: Style,
    single_line_conditions_only: bool,
    include_ternary_expressions: bool,
    /// `Layout/EndAlignment`'s `EnforcedStyleAlignWith == 'keyword'`
    /// (RuboCop's real default).
    end_align_with_keyword: bool,
    /// `Layout/LineLength`'s `Max`, or `None` when that cop is disabled --
    /// RuboCop's `correction_exceeds_line_limit?` gate.
    max_line_length: Option<i64>,
    /// RuboCop's `ignore_node` set: every candidate assignment's own span,
    /// so a nested assignment is skipped by `part_of_ignored_node?`.
    ignored: Vec<Span>,
}

/// One of the four conditional shapes this cop recognizes as a candidate
/// right-hand side: `if`/ternary (Prism folds both into
/// [`ruby_ast::node::IfNode`], distinguished by [`IfNode::if_keyword_loc`]),
/// `unless`, `case`/`when`, and `case`/`in`.
#[derive(Clone, Copy)]
enum Shape<'pr> {
    If(IfNode<'pr>),
    Unless(UnlessNode<'pr>),
    Case(CaseNode<'pr>),
    CaseMatch(CaseMatchNode<'pr>),
}

impl<'pr> Shape<'pr> {
    fn classify(node: &Node<'pr>) -> Option<Self> {
        match node.kind() {
            NodeKind::IfNode => node.as_if_node().map(Shape::If),
            NodeKind::UnlessNode => node.as_unless_node().map(Shape::Unless),
            NodeKind::CaseNode => node.as_case_node().map(Shape::Case),
            NodeKind::CaseMatchNode => node.as_case_match_node().map(Shape::CaseMatch),
            _ => None,
        }
    }

    /// RuboCop's `allowed_ternary?`'s own condition: no `if`/`elsif`
    /// keyword at all (ternary syntax admits no `elsif`, and `unless`/
    /// `case` have no ternary form).
    fn is_ternary(self) -> bool {
        matches!(self, Shape::If(n) if n.if_keyword_loc().is_none())
    }

    /// Whether this conditional has a real continuation: an `else`/`elsif`
    /// (if/unless) or an `else` clause (case). RuboCop's
    /// `assignment.else_branch`/`node.else_branch`.
    fn has_continuation(self) -> bool {
        match self {
            Shape::If(n) => n.subsequent().is_some(),
            Shape::Unless(n) => n.else_clause().is_some(),
            Shape::Case(n) => n.else_clause().is_some(),
            Shape::CaseMatch(n) => n.else_clause().is_some(),
        }
    }

    fn span(self) -> Span {
        match self {
            Shape::If(n) => n.as_node().span(),
            Shape::Unless(n) => n.as_node().span(),
            Shape::Case(n) => n.as_node().span(),
            Shape::CaseMatch(n) => n.as_node().span(),
        }
    }

    /// RuboCop's `end_keyword_loc`-ish accessor for whichever node kind
    /// owns the terminal `end` (a ternary has none).
    fn end_keyword(self) -> Option<Span> {
        match self {
            Shape::If(n) => n.end_keyword_loc().map(|l| l.span()),
            Shape::Unless(n) => n.end_keyword_loc().map(|l| l.span()),
            Shape::Case(n) => Some(n.end_keyword_loc().span()),
            Shape::CaseMatch(n) => Some(n.end_keyword_loc().span()),
        }
    }

    /// RuboCop's `allowed_single_line?`: true when at least one of this
    /// conditional's *outer* branch slots is a multi-statement body. Like
    /// upstream, this only ever inspects the head (`if`) branch and the
    /// final `else` slot for `if`/`unless` (an `elsif` link's own slot is
    /// itself a nested conditional node, never a multi-statement body, so
    /// it never contributes) and only the final `else` slot for `case`
    /// (each `when`/`in` is a distinct node type from its body, likewise
    /// never counted) -- reproducing that same asymmetry rather than a
    /// more "correct" scan of every branch.
    fn any_multi_statement_branch(self) -> bool {
        fn is_multi(stmts: Option<StatementsNode<'_>>) -> bool {
            stmts.is_some_and(|s| s.body().len() > 1)
        }
        match self {
            Shape::If(n) => {
                is_multi(n.statements())
                    || n.subsequent()
                        .is_some_and(|s| s.as_else_node().is_some_and(|e| is_multi(e.statements())))
            }
            Shape::Unless(n) => {
                is_multi(n.statements()) || is_multi(n.else_clause().and_then(|e| e.statements()))
            }
            Shape::Case(n) => is_multi(n.else_clause().and_then(|e| e.statements())),
            Shape::CaseMatch(n) => is_multi(n.else_clause().and_then(|e| e.statements())),
        }
    }
}

/// RuboCop's `end_with_eq?` plus the rest of `assignment_type?`'s `send`
/// alternation: `[]=`, `<<`, `=~`, `!~`, `<=>`, `<`, `>`, or any method name
/// ending in `=` (this literally includes `==`/`!=`/`<=`/`>=`/`===` too,
/// matching upstream's own node-pattern exactly).
fn is_assignment_call(call: &CallNode<'_>) -> bool {
    let name = call.name();
    let name = name.as_slice();
    matches!(name, b"[]=" | b"<<" | b"=~" | b"!~" | b"<=>" | b"<" | b">") || name.ends_with(b"=")
}

/// The right-hand side of any node this cop treats as an assignment:
/// every writable-target node kind Prism has (a plain write, or an
/// `op_asgn`/`and_asgn`/`or_asgn` wrapper around a local/instance/class/
/// global variable, a constant (bare or namespaced), an index, or a method
/// call), plus a `send` matching [`is_assignment_call`] (element/attribute
/// writers and the comparison-operator forms upstream's pattern admits),
/// read off its last positional argument. `None` for a safe-navigation
/// call (whitequark's `csend`, never matched by upstream's `(send ...)`
/// pattern) or one with no arguments at all.
fn rhs_of<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    macro_rules! val {
        ($accessor:ident) => {
            node.$accessor().map(|n| n.value())
        };
    }
    match node.kind() {
        NodeKind::LocalVariableWriteNode => val!(as_local_variable_write_node),
        NodeKind::InstanceVariableWriteNode => val!(as_instance_variable_write_node),
        NodeKind::ClassVariableWriteNode => val!(as_class_variable_write_node),
        NodeKind::GlobalVariableWriteNode => val!(as_global_variable_write_node),
        NodeKind::ConstantWriteNode => val!(as_constant_write_node),
        NodeKind::ConstantPathWriteNode => val!(as_constant_path_write_node),
        NodeKind::MultiWriteNode => val!(as_multi_write_node),
        NodeKind::LocalVariableOperatorWriteNode => val!(as_local_variable_operator_write_node),
        NodeKind::LocalVariableAndWriteNode => val!(as_local_variable_and_write_node),
        NodeKind::LocalVariableOrWriteNode => val!(as_local_variable_or_write_node),
        NodeKind::InstanceVariableOperatorWriteNode => {
            val!(as_instance_variable_operator_write_node)
        }
        NodeKind::InstanceVariableAndWriteNode => val!(as_instance_variable_and_write_node),
        NodeKind::InstanceVariableOrWriteNode => val!(as_instance_variable_or_write_node),
        NodeKind::ClassVariableOperatorWriteNode => val!(as_class_variable_operator_write_node),
        NodeKind::ClassVariableAndWriteNode => val!(as_class_variable_and_write_node),
        NodeKind::ClassVariableOrWriteNode => val!(as_class_variable_or_write_node),
        NodeKind::GlobalVariableOperatorWriteNode => val!(as_global_variable_operator_write_node),
        NodeKind::GlobalVariableAndWriteNode => val!(as_global_variable_and_write_node),
        NodeKind::GlobalVariableOrWriteNode => val!(as_global_variable_or_write_node),
        NodeKind::ConstantOperatorWriteNode => val!(as_constant_operator_write_node),
        NodeKind::ConstantAndWriteNode => val!(as_constant_and_write_node),
        NodeKind::ConstantOrWriteNode => val!(as_constant_or_write_node),
        NodeKind::ConstantPathOperatorWriteNode => val!(as_constant_path_operator_write_node),
        NodeKind::ConstantPathAndWriteNode => val!(as_constant_path_and_write_node),
        NodeKind::ConstantPathOrWriteNode => val!(as_constant_path_or_write_node),
        NodeKind::IndexOperatorWriteNode => val!(as_index_operator_write_node),
        NodeKind::IndexAndWriteNode => val!(as_index_and_write_node),
        NodeKind::IndexOrWriteNode => val!(as_index_or_write_node),
        NodeKind::CallOperatorWriteNode => val!(as_call_operator_write_node),
        NodeKind::CallAndWriteNode => val!(as_call_and_write_node),
        NodeKind::CallOrWriteNode => val!(as_call_or_write_node),
        NodeKind::CallNode => {
            let call = node.as_call_node()?;
            if call.is_safe_navigation() || !is_assignment_call(&call) {
                return None;
            }
            call.arguments()?.arguments().last()
        }
        _ => None,
    }
}

/// RuboCop's `elsif?`: an `IfNode` reached as another `IfNode`'s
/// `subsequent`, told apart from a genuine (if nested) `if` only by its
/// own `if_keyword_loc` reading `"elsif"`.
fn is_elsif(if_node: &IfNode<'_>) -> bool {
    if_node.if_keyword_loc().is_some_and(|l| l.as_slice() == b"elsif")
}

/// The single-statement then/else expressions of a ternary (`a ? b : c`),
/// RuboCop's `TernaryCorrector.extract_branches`.
fn ternary_parts<'pr>(if_node: &IfNode<'pr>) -> Option<(Node<'pr>, Node<'pr>)> {
    let if_branch = if_node.statements()?.body().first()?;
    let else_node = if_node.subsequent()?.as_else_node()?;
    let else_branch = else_node.statements()?.body().first()?;
    Some((if_branch, else_branch))
}

// ---------------------------------------------------------------------
// `assign_inside_condition`: move the assignment from outside the
// conditional into each branch.
// ---------------------------------------------------------------------

/// One `if`/`unless`/`case`/`case`-`in`'s branches for the
/// `assign_inside_condition` corrector: every branch's body (`None` for an
/// empty branch, dropped like RuboCop's `branches.flatten`), every
/// `elsif`/`else`/`when`/`in` keyword needing its indentation reset (the
/// head `if`/`case`/`unless` keyword is harmlessly included too -- it is
/// always on the same source line as the assignment being removed, so the
/// `same_line?` guard in [`build_correction`] always skips it), and the
/// terminal `end` keyword's own location.
struct Parts<'pr> {
    branches: Vec<Option<StatementsNode<'pr>>>,
    keywords: Vec<Span>,
    end_keyword: Option<Span>,
}

fn if_parts<'pr>(head: &IfNode<'pr>) -> Parts<'pr> {
    let mut branches = Vec::new();
    let mut keywords = Vec::new();
    if let Some(kw) = head.if_keyword_loc() {
        keywords.push(kw.span());
    }
    let mut current = *head;
    loop {
        branches.push(current.statements());
        match current.subsequent() {
            Some(node) => {
                if let Some(elsif) = node.as_if_node() {
                    if let Some(kw) = elsif.if_keyword_loc() {
                        keywords.push(kw.span());
                    }
                    current = elsif;
                } else if let Some(else_node) = node.as_else_node() {
                    keywords.push(else_node.else_keyword_loc().span());
                    branches.push(else_node.statements());
                    break;
                } else {
                    break;
                }
            }
            None => break,
        }
    }
    Parts { branches, keywords, end_keyword: head.end_keyword_loc().map(|l| l.span()) }
}

fn unless_parts<'pr>(n: &UnlessNode<'pr>) -> Parts<'pr> {
    let mut branches = vec![n.statements()];
    let mut keywords = vec![n.keyword_loc().span()];
    if let Some(else_node) = n.else_clause() {
        keywords.push(else_node.else_keyword_loc().span());
        branches.push(else_node.statements());
    }
    Parts { branches, keywords, end_keyword: n.end_keyword_loc().map(|l| l.span()) }
}

fn case_like_parts<'pr>(
    case_keyword: Span,
    conditions: &NodeList<'pr>,
    else_clause: Option<ElseNode<'pr>>,
    end_keyword: Span,
) -> Parts<'pr> {
    let mut branches = Vec::new();
    let mut keywords = vec![case_keyword];
    for cond in conditions {
        if let Some(when) = cond.as_when_node() {
            keywords.push(when.keyword_loc().span());
            branches.push(when.statements());
        } else if let Some(in_node) = cond.as_in_node() {
            keywords.push(in_node.in_loc().span());
            branches.push(in_node.statements());
        }
    }
    if let Some(else_node) = else_clause {
        keywords.push(else_node.else_keyword_loc().span());
        branches.push(else_node.statements());
    }
    Parts { branches, keywords, end_keyword: Some(end_keyword) }
}

fn parts_of(shape: Shape<'_>) -> Parts<'_> {
    match shape {
        Shape::If(n) => if_parts(&n),
        Shape::Unless(n) => unless_parts(&n),
        Shape::Case(n) => case_like_parts(
            n.case_keyword_loc().span(),
            &n.conditions(),
            n.else_clause(),
            n.end_keyword_loc().span(),
        ),
        Shape::CaseMatch(n) => case_like_parts(
            n.case_keyword_loc().span(),
            &n.conditions(),
            n.else_clause(),
            n.end_keyword_loc().span(),
        ),
    }
}

/// RuboCop's `assignment_node`'s `begin`-unwrapping step: where whitequark
/// hands upstream a one-child `begin` for `x = (cond ? a : b)`, Prism hands
/// a [`ruby_ast::node::ParenthesesNode`].
fn unwrap_parens(node: Node<'_>) -> Node<'_> {
    let Some(parens) = node.as_parentheses_node() else { return node };
    let Some(stmts) = parens.body().and_then(|b| b.as_statements_node()) else { return node };
    let body = stmts.body();
    if body.len() == 1 {
        body.iter().next().unwrap_or(node)
    } else {
        node
    }
}

/// RuboCop's `ConditionalCorrectorHelper#white_space_range`: the run of
/// `node.column - column - 2` bytes immediately preceding `node_start`,
/// but only when it is pure whitespace (and non-empty -- upstream's
/// `corrector.remove` of an empty range is a no-op).
fn white_space_range(node_start: u32, column: u32, ctx: &Context<'_>) -> Option<Span> {
    let len = ctx.line_col(node_start).column.checked_sub(column + 2)?;
    if len == 0 {
        return None;
    }
    let begin = node_start.checked_sub(len)?;
    let span = Span::new(begin, node_start);
    ctx.text(span).iter().all(u8::is_ascii_whitespace).then_some(span)
}

/// RuboCop's `remove_whitespace_in_branches`'s `branch.each_node` walk:
/// every node in the branch (itself included) whose own indentation can be
/// pulled back to `column + 2` is dedented.
///
/// `parent_is_dstr` reproduces upstream's `next if child.parent.dstr_type?`
/// guard, which is what keeps the continuation lines of a multi-line
/// interpolated string (or an interpolated heredoc's body) from being
/// dedented. [`NodeKind::ElseNode`] and [`NodeKind::EnsureNode`] are
/// skipped because they are Prism-only wrappers whose location starts at
/// the `else`/`ensure` keyword; whitequark has no node there, so upstream
/// never dedents a *nested* conditional's `else` line.
fn collect_branch_whitespace(
    node: &Node<'_>,
    parent_is_dstr: bool,
    column: u32,
    ctx: &Context<'_>,
    edits: &mut Vec<Edit>,
) {
    if !parent_is_dstr && !matches!(node.kind(), NodeKind::ElseNode | NodeKind::EnsureNode) {
        if let Some(span) = white_space_range(node.span().start, column, ctx) {
            edits.push(Edit::delete(span));
        }
    }
    let is_dstr = node.kind() == NodeKind::InterpolatedStringNode;
    ruby_ast::for_each_child(node, |child| {
        collect_branch_whitespace(child, is_dstr, column, ctx, edits);
    });
}

/// RuboCop's `corrector.remove_preceding(loc, loc.column - column)`: blindly
/// removes the byte run immediately preceding `keyword_start` down to
/// `column` columns of indentation (no purity check -- upstream assumes a
/// keyword like `else`/`end` is preceded by plain indentation).
fn dedent_keyword(keyword_start: u32, column: u32, ctx: &Context<'_>, edits: &mut Vec<Edit>) {
    let Some(len) = ctx.line_col(keyword_start).column.checked_sub(column) else { return };
    if len == 0 {
        return;
    }
    let Some(begin) = keyword_start.checked_sub(len) else { return };
    edits.push(Edit::delete(Span::new(begin, keyword_start)));
}

/// RuboCop's `IfCorrector`/`CaseCorrector` `move_assignment_inside_condition`:
/// inserts `assignment_text` before each branch's tail statement, dedents
/// each branch's nodes to `column + 2`, and dedents every `elsif`/`else`/
/// `when`/`in` keyword not on the conditional's own line plus the terminal
/// `end` when it is not on the final `else` body's line.
///
/// Returns `false` for a branch slot upstream would blow up on (`tail(nil)`
/// raises `NoMethodError`, which `safe_to_correct?` turns into "no
/// offense").
fn build_correction(
    parts: &Parts<'_>,
    condition_start: u32,
    assignment_text: &[u8],
    column: u32,
    ctx: &Context<'_>,
    edits: &mut Vec<Edit>,
) -> bool {
    let condition_line = ctx.line_col(condition_start).line;
    let mut last_branch_line = None;
    for branch in &parts.branches {
        let Some(branch) = *branch else { return false };
        let Some(tail) = branch.body().last() else { return false };
        edits.push(Edit::insert(tail.span().start, assignment_text.to_vec()));
        let branch_node = branch.as_node();
        collect_branch_whitespace(&branch_node, false, column, ctx, edits);
        last_branch_line = Some(ctx.line_col(branch_node.span().start).line);
    }
    for &kw in &parts.keywords {
        if ctx.line_col(kw.start).line != condition_line {
            dedent_keyword(kw.start, column, ctx, edits);
        }
    }
    if let Some(end) = parts.end_keyword {
        if last_branch_line != Some(ctx.line_col(end.start).line) {
            dedent_keyword(end.start, column, ctx, edits);
        }
    }
    true
}

/// Parser's `TreeRewriter` policies RuboCop's `Corrector` is built with:
/// `crossing_deletions: :accept` (overlapping removals merge into their
/// union) and `swallowed_insertions: :raise` (an insertion strictly inside
/// a removal aborts the correction, which `safe_to_correct?` turns into
/// "no offense"). `None` is that abort.
fn merge_edits(mut edits: Vec<Edit>) -> Option<Vec<Edit>> {
    edits.sort_by_key(|edit| (edit.span.start, edit.span.end));
    let mut out: Vec<Edit> = Vec::with_capacity(edits.len());
    for edit in edits {
        let is_removal = edit.span.start < edit.span.end && edit.replacement.is_empty();
        match out.last_mut() {
            Some(last)
                if is_removal
                    && last.replacement.is_empty()
                    && last.span.start < last.span.end
                    && edit.span.start <= last.span.end =>
            {
                last.span.end = last.span.end.max(edit.span.end);
            }
            Some(last) if last.span.start < edit.span.start && edit.span.start < last.span.end => {
                return None;
            }
            _ => out.push(edit),
        }
    }
    Some(out)
}

/// RuboCop's `safe_to_correct?`/`ReparsedEquivalence#correction_parses?`:
/// the correction is applied to a throwaway copy of the source and the
/// result must still parse.
fn correction_parses(edits: &[Edit], ctx: &Context<'_>) -> bool {
    let original = ctx.source().bytes();
    let mut corrected = Vec::with_capacity(original.len());
    let mut pos = 0usize;
    for edit in edits {
        let range = edit.span.range();
        if range.start < pos || range.end > original.len() {
            return false;
        }
        corrected.extend_from_slice(&original[pos..range.start]);
        corrected.extend_from_slice(&edit.replacement);
        pos = range.end;
    }
    corrected.extend_from_slice(&original[pos..]);
    let source = SourceFile::new("(conditional_assignment)", corrected);
    let parsed = Parsed::parse(&source);
    !parsed.has_errors()
}

/// Emits the offense once `edits` has cleared upstream's `safe_to_correct?`
/// gate. An empty edit list is upstream's "no corrector matched" case (a
/// parenthesized non-ternary conditional), which still reports.
fn report_checked(span: Span, message: &'static str, edits: Vec<Edit>, ctx: &mut Context<'_>) {
    let Some(edits) = merge_edits(edits) else { return };
    if !correction_parses(&edits, ctx) {
        return;
    }
    if edits.is_empty() {
        ctx.report(&<ConditionalAssignment as Rule>::META, span, message);
    } else {
        ctx.report_with_fix(
            &<ConditionalAssignment as Rule>::META,
            span,
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

/// RuboCop's `check_assignment_to_condition` plus
/// `IfCorrector`/`CaseCorrector`/`TernaryCorrector`
/// `.move_assignment_inside_condition`: the `assign_inside_condition`
/// style.
fn check_assignment_inside(
    node: &Node<'_>,
    style: Style,
    include_ternary_expressions: bool,
    single_line_conditions_only: bool,
    ignored: &mut Vec<Span>,
    ctx: &mut Context<'_>,
) {
    if style != Style::AssignInsideCondition {
        return;
    }
    if let Some(call) = node.as_call_node() {
        if call.is_safe_navigation() || !is_assignment_call(&call) {
            return;
        }
    }
    let node_span = node.span();
    // RuboCop's `part_of_ignored_node?`: every candidate assignment calls
    // `ignore_node`, so an assignment nested in another assignment's
    // right-hand side is never considered.
    if ignored.iter().any(|s| s.start <= node_span.start && node_span.end <= s.end) {
        return;
    }
    ignored.push(node_span);

    let Some(raw) = rhs_of(node) else { return };
    let assignment = unwrap_parens(raw);
    let Some(shape) = Shape::classify(&assignment) else { return };
    if shape.is_ternary() && !include_ternary_expressions {
        return;
    }
    if !shape.has_continuation() {
        return;
    }
    if single_line_conditions_only && shape.any_multi_statement_branch() {
        return;
    }

    let column = ctx.line_col(node_span.start).column;
    let assignment_span = Span::new(node_span.start, raw.span().start);
    let assignment_text = ctx.text(assignment_span).to_vec();
    let parenthesized = raw.as_parentheses_node();

    let mut edits = Vec::new();
    if let (Shape::If(if_node), true) = (shape, shape.is_ternary()) {
        if let Some(parens) = parenthesized {
            edits.push(Edit::delete(parens.opening_loc().span()));
            edits.push(Edit::delete(parens.closing_loc().span()));
        }
        edits.push(Edit::delete(assignment_span));
        let Some((if_branch, else_branch)) = ternary_parts(&if_node) else { return };
        edits.push(Edit::insert(if_branch.span().start, assignment_text.clone()));
        edits.push(Edit::insert(else_branch.span().start, assignment_text));
    } else if parenthesized.is_none() {
        // Upstream dispatches on the *raw* right-hand side, so a
        // parenthesized non-ternary conditional matches no corrector at all
        // and the offense is reported without a fix.
        edits.push(Edit::delete(assignment_span));
        let parts = parts_of(shape);
        if !build_correction(
            &parts,
            assignment.span().start,
            &assignment_text,
            column,
            ctx,
            &mut edits,
        ) {
            return;
        }
    }

    report_checked(node_span, ASSIGN_TO_CONDITION_MSG, edits, ctx);
}

// ---------------------------------------------------------------------
// `assign_to_condition`: move the assignment from inside each branch to
// outside the conditional.
// ---------------------------------------------------------------------

/// RuboCop's expanded branch-body list for `check_node`: `on_if`'s
/// `[node.if_branch, *elsif_branches, else_branch]` (via `expand_elses`),
/// `on_case`/`on_case_match`'s `[*expand_when_branches(...), else_branch]`.
/// `None` for an empty branch, which `allowed_statements?`'s
/// `branches.all?` rejects.
///
/// `if_branch_bodies` returns `None` when the chain has no terminal `else`
/// (`expand_elses` pops a `nil` off the `elsif` list and `on_if` bails).
fn if_branch_bodies<'pr>(head: &IfNode<'pr>) -> Option<Vec<Option<StatementsNode<'pr>>>> {
    let mut out = vec![head.statements()];
    let mut current = *head;
    loop {
        let node = current.subsequent()?;
        if let Some(elsif) = node.as_if_node() {
            out.push(elsif.statements());
            current = elsif;
        } else {
            let else_node = node.as_else_node()?;
            out.push(else_node.statements());
            return Some(out);
        }
    }
}

fn unless_branch_bodies<'pr>(n: &UnlessNode<'pr>) -> Vec<Option<StatementsNode<'pr>>> {
    vec![n.statements(), n.else_clause().and_then(|e| e.statements())]
}

fn case_branch_bodies<'pr>(
    conditions: &NodeList<'pr>,
    else_clause: Option<ElseNode<'pr>>,
) -> Vec<Option<StatementsNode<'pr>>> {
    let mut out = Vec::new();
    for cond in conditions {
        if let Some(when) = cond.as_when_node() {
            out.push(when.statements());
        } else if let Some(in_node) = cond.as_in_node() {
            out.push(in_node.statements());
        }
    }
    out.push(else_clause.and_then(|e| e.statements()));
    out
}

/// `target op= ` (upstream's `"#{node.assignment_node.source} #{node.operator}= "`),
/// given the target's own raw source text and the bare operator (`+`,
/// `&&`, `||`, ...).
fn op_lhs(target: &[u8], op: &[u8]) -> Vec<u8> {
    let mut out = target.to_vec();
    out.push(b' ');
    out.extend_from_slice(op);
    out.extend_from_slice(b"= ");
    out
}

/// `name = ` for a plain variable/bare-constant write.
fn plain_lhs(name: &[u8]) -> Vec<u8> {
    let mut out = name.to_vec();
    out.extend_from_slice(b" = ");
    out
}

/// `path = ` for a namespaced constant write (RuboCop's `lhs_for_casgn`,
/// read directly off the constant path's own source rather than
/// reconstructed piece by piece -- equivalent for any path a parser would
/// ever produce).
fn plain_lhs_span(span: Span, ctx: &Context<'_>) -> Vec<u8> {
    plain_lhs(ctx.text(span))
}

/// `receiver[args] ` (no trailing `=`; callers append it) for an index
/// target, RuboCop's aref half of `lhs_for_send` generalized to the
/// `op_asgn`/`and_asgn`/`or_asgn` index-write node kinds.
fn index_target_text(
    receiver: Option<Node<'_>>,
    arguments: Option<ArgumentsNode<'_>>,
    ctx: &Context<'_>,
) -> Vec<u8> {
    let mut out = receiver.map(|r| ctx.text(r.span()).to_vec()).unwrap_or_default();
    out.push(b'[');
    if let Some(args) = arguments {
        for (i, arg) in args.arguments().iter().enumerate() {
            if i > 0 {
                out.extend_from_slice(b", ");
            }
            out.extend_from_slice(ctx.text(arg.span()));
        }
    }
    out.push(b']');
    out
}

/// `receiver.reader_name` for an attribute target, generalized to the
/// `op_asgn`/`and_asgn`/`or_asgn` call-write node kinds (which expose the
/// reader form directly via `read_name`, unlike a plain `send` where it
/// must be sliced off the writer method name -- see [`send_lhs`]).
fn call_target_text(receiver: Option<Node<'_>>, read_name: &[u8], ctx: &Context<'_>) -> Vec<u8> {
    let mut out = receiver.map(|r| ctx.text(r.span()).to_vec()).unwrap_or_default();
    out.push(b'.');
    out.extend_from_slice(read_name);
    out
}

/// RuboCop-AST's `assignment_method?`-ish setter check for `lhs_for_send`:
/// a method name ending in `=` that is neither the aref writer (`[]=`,
/// handled separately) nor one of the comparison operators that
/// incidentally also end in `=`.
fn is_setter(name: &[u8]) -> bool {
    name.ends_with(b"=")
        && name != b"[]="
        && !matches!(name, b"==" | b"!=" | b"<=" | b">=" | b"===")
}

/// RuboCop's `lhs_for_send`: `receiver[i, ...] = ` for `[]=`,
/// `receiver.attr = ` for a setter, else `receiver method_name ` (covers
/// `<<`, `=~`, `!~`, `<=>`, `<`, `>`, and any comparison-operator name that
/// happens to end in `=`).
fn send_lhs(call: CallNode<'_>, ctx: &Context<'_>) -> Vec<u8> {
    let receiver = call.receiver().map(|r| ctx.text(r.span()).to_vec()).unwrap_or_default();
    let name = call.name();
    let name = name.as_slice();
    if name == b"[]=" {
        let mut out = receiver;
        out.push(b'[');
        if let Some(args) = call.arguments() {
            let list = args.arguments();
            let n = list.len();
            for (i, arg) in list.iter().enumerate() {
                if i + 1 == n {
                    break;
                }
                if i > 0 {
                    out.extend_from_slice(b", ");
                }
                out.extend_from_slice(ctx.text(arg.span()));
            }
        }
        out.extend_from_slice(b"] = ");
        out
    } else if is_setter(name) {
        let mut out = receiver;
        out.push(b'.');
        out.extend_from_slice(&name[..name.len() - 1]);
        out.extend_from_slice(b" = ");
        out
    } else {
        let mut out = receiver;
        out.push(b' ');
        out.extend_from_slice(name);
        out.push(b' ');
        out
    }
}

/// RuboCop's `lhs`: the reconstructed assignment prefix for any recognized
/// assignment-shaped node. The `_` fallback (whole-node source) is
/// RuboCop's own `else: node.source` arm, reached in practice only for
/// `masgn` -- which never gets this far, since [`check_outer`] rejects a
/// `MultiWriteNode` first tail before ever calling this.
fn lhs(node: &Node<'_>, ctx: &Context<'_>) -> Vec<u8> {
    if let Some(text) = lhs_plain_write(node, ctx) {
        return text;
    }
    if let Some(text) = lhs_operator_write(node, ctx) {
        return text;
    }
    if let Some(text) = lhs_and_write(node, ctx) {
        return text;
    }
    if let Some(text) = lhs_or_write(node, ctx) {
        return text;
    }
    if let Some(text) = lhs_index_write(node, ctx) {
        return text;
    }
    if let Some(text) = lhs_call_target_write(node, ctx) {
        return text;
    }
    match node.kind() {
        NodeKind::CallNode => send_lhs(node.as_call_node().expect("kind matched"), ctx),
        _ => ctx.text(node.span()).to_vec(),
    }
}

/// [`lhs`]'s arm for a plain variable/bare-or-namespaced-constant write (no
/// compound operator). `None` for any other node kind.
fn lhs_plain_write(node: &Node<'_>, ctx: &Context<'_>) -> Option<Vec<u8>> {
    Some(match node.kind() {
        NodeKind::LocalVariableWriteNode => plain_lhs(
            node.as_local_variable_write_node().expect("kind matched").name_loc().as_slice(),
        ),
        NodeKind::InstanceVariableWriteNode => plain_lhs(
            node.as_instance_variable_write_node().expect("kind matched").name_loc().as_slice(),
        ),
        NodeKind::ClassVariableWriteNode => plain_lhs(
            node.as_class_variable_write_node().expect("kind matched").name_loc().as_slice(),
        ),
        NodeKind::GlobalVariableWriteNode => plain_lhs(
            node.as_global_variable_write_node().expect("kind matched").name_loc().as_slice(),
        ),
        NodeKind::ConstantWriteNode => {
            plain_lhs(node.as_constant_write_node().expect("kind matched").name_loc().as_slice())
        }
        NodeKind::ConstantPathWriteNode => plain_lhs_span(
            node.as_constant_path_write_node().expect("kind matched").target().as_node().span(),
            ctx,
        ),
        _ => return None,
    })
}

/// [`lhs`]'s arm for an `op_asgn` (`+=`, `-=`, ...) write. `None` for any
/// other node kind.
fn lhs_operator_write(node: &Node<'_>, ctx: &Context<'_>) -> Option<Vec<u8>> {
    Some(match node.kind() {
        NodeKind::LocalVariableOperatorWriteNode => {
            let n = node.as_local_variable_operator_write_node().expect("kind matched");
            op_lhs(n.name_loc().as_slice(), n.binary_operator().as_slice())
        }
        NodeKind::InstanceVariableOperatorWriteNode => {
            let n = node.as_instance_variable_operator_write_node().expect("kind matched");
            op_lhs(n.name_loc().as_slice(), n.binary_operator().as_slice())
        }
        NodeKind::ClassVariableOperatorWriteNode => {
            let n = node.as_class_variable_operator_write_node().expect("kind matched");
            op_lhs(n.name_loc().as_slice(), n.binary_operator().as_slice())
        }
        NodeKind::GlobalVariableOperatorWriteNode => {
            let n = node.as_global_variable_operator_write_node().expect("kind matched");
            op_lhs(n.name_loc().as_slice(), n.binary_operator().as_slice())
        }
        NodeKind::ConstantOperatorWriteNode => {
            let n = node.as_constant_operator_write_node().expect("kind matched");
            op_lhs(n.name_loc().as_slice(), n.binary_operator().as_slice())
        }
        NodeKind::ConstantPathOperatorWriteNode => {
            let n = node.as_constant_path_operator_write_node().expect("kind matched");
            op_lhs(ctx.text(n.target().as_node().span()), n.binary_operator().as_slice())
        }
        _ => return None,
    })
}

/// [`lhs`]'s arm for an `and_asgn` (`||=`... no, `&&=`) write. `None` for
/// any other node kind.
fn lhs_and_write(node: &Node<'_>, ctx: &Context<'_>) -> Option<Vec<u8>> {
    Some(match node.kind() {
        NodeKind::LocalVariableAndWriteNode => op_lhs(
            node.as_local_variable_and_write_node().expect("kind matched").name_loc().as_slice(),
            b"&&",
        ),
        NodeKind::InstanceVariableAndWriteNode => op_lhs(
            node.as_instance_variable_and_write_node().expect("kind matched").name_loc().as_slice(),
            b"&&",
        ),
        NodeKind::ClassVariableAndWriteNode => op_lhs(
            node.as_class_variable_and_write_node().expect("kind matched").name_loc().as_slice(),
            b"&&",
        ),
        NodeKind::GlobalVariableAndWriteNode => op_lhs(
            node.as_global_variable_and_write_node().expect("kind matched").name_loc().as_slice(),
            b"&&",
        ),
        NodeKind::ConstantAndWriteNode => op_lhs(
            node.as_constant_and_write_node().expect("kind matched").name_loc().as_slice(),
            b"&&",
        ),
        NodeKind::ConstantPathAndWriteNode => op_lhs(
            ctx.text(
                node.as_constant_path_and_write_node()
                    .expect("kind matched")
                    .target()
                    .as_node()
                    .span(),
            ),
            b"&&",
        ),
        _ => return None,
    })
}

/// [`lhs`]'s arm for an `or_asgn` (`||=`) write. `None` for any other node
/// kind.
fn lhs_or_write(node: &Node<'_>, ctx: &Context<'_>) -> Option<Vec<u8>> {
    Some(match node.kind() {
        NodeKind::LocalVariableOrWriteNode => op_lhs(
            node.as_local_variable_or_write_node().expect("kind matched").name_loc().as_slice(),
            b"||",
        ),
        NodeKind::InstanceVariableOrWriteNode => op_lhs(
            node.as_instance_variable_or_write_node().expect("kind matched").name_loc().as_slice(),
            b"||",
        ),
        NodeKind::ClassVariableOrWriteNode => op_lhs(
            node.as_class_variable_or_write_node().expect("kind matched").name_loc().as_slice(),
            b"||",
        ),
        NodeKind::GlobalVariableOrWriteNode => op_lhs(
            node.as_global_variable_or_write_node().expect("kind matched").name_loc().as_slice(),
            b"||",
        ),
        NodeKind::ConstantOrWriteNode => op_lhs(
            node.as_constant_or_write_node().expect("kind matched").name_loc().as_slice(),
            b"||",
        ),
        NodeKind::ConstantPathOrWriteNode => op_lhs(
            ctx.text(
                node.as_constant_path_or_write_node()
                    .expect("kind matched")
                    .target()
                    .as_node()
                    .span(),
            ),
            b"||",
        ),
        _ => return None,
    })
}

/// [`lhs`]'s arm for an index (`[]`) write, plain/`op_asgn`/`and_asgn`/
/// `or_asgn` alike. `None` for any other node kind.
fn lhs_index_write(node: &Node<'_>, ctx: &Context<'_>) -> Option<Vec<u8>> {
    Some(match node.kind() {
        NodeKind::IndexOperatorWriteNode => {
            let n = node.as_index_operator_write_node().expect("kind matched");
            op_lhs(
                &index_target_text(n.receiver(), n.arguments(), ctx),
                n.binary_operator().as_slice(),
            )
        }
        NodeKind::IndexAndWriteNode => {
            let n = node.as_index_and_write_node().expect("kind matched");
            op_lhs(&index_target_text(n.receiver(), n.arguments(), ctx), b"&&")
        }
        NodeKind::IndexOrWriteNode => {
            let n = node.as_index_or_write_node().expect("kind matched");
            op_lhs(&index_target_text(n.receiver(), n.arguments(), ctx), b"||")
        }
        _ => return None,
    })
}

/// [`lhs`]'s arm for an attribute (`receiver.attr`) write,
/// `op_asgn`/`and_asgn`/`or_asgn` alike (a plain attribute write is a
/// `send`, handled by [`send_lhs`] instead). `None` for any other node
/// kind.
fn lhs_call_target_write(node: &Node<'_>, ctx: &Context<'_>) -> Option<Vec<u8>> {
    Some(match node.kind() {
        NodeKind::CallOperatorWriteNode => {
            let n = node.as_call_operator_write_node().expect("kind matched");
            op_lhs(
                &call_target_text(n.receiver(), n.read_name().as_slice(), ctx),
                n.binary_operator().as_slice(),
            )
        }
        NodeKind::CallAndWriteNode => {
            let n = node.as_call_and_write_node().expect("kind matched");
            op_lhs(&call_target_text(n.receiver(), n.read_name().as_slice(), ctx), b"&&")
        }
        NodeKind::CallOrWriteNode => {
            let n = node.as_call_or_write_node().expect("kind matched");
            op_lhs(&call_target_text(n.receiver(), n.read_name().as_slice(), ctx), b"||")
        }
        _ => return None,
    })
}

/// RuboCop's `replace_branch_assignment`: a branch's tail replaced with its
/// own RHS source, bracketed when that RHS is an unbracketed array literal
/// (`bar = 2, 5, 6`).
fn tail_rhs_replacement(tail: &Node<'_>, ctx: &Context<'_>) -> Vec<u8> {
    let Some(rhs) = rhs_of(tail) else { return ctx.text(tail.span()).to_vec() };
    let text = ctx.text(rhs.span());
    if let Some(arr) = rhs.as_array_node() {
        if arr.opening_loc().is_none() {
            let mut wrapped = Vec::with_capacity(text.len() + 2);
            wrapped.push(b'[');
            wrapped.extend_from_slice(text);
            wrapped.push(b']');
            return wrapped;
        }
    }
    text.to_vec()
}

/// RuboCop's `correct_branches`: a branch's tail replaced with its own RHS
/// source, no bracket check.
fn plain_rhs_replacement(tail: &Node<'_>, ctx: &Context<'_>) -> Vec<u8> {
    rhs_of(tail).map_or_else(|| ctx.text(tail.span()).to_vec(), |rhs| ctx.text(rhs.span()).to_vec())
}

/// RuboCop's `IfCorrector.correct`/`CaseCorrector.correct`: inserts
/// `lhs_text` before the conditional, replaces every branch's tail with its
/// RHS (bracket-checked for the head and final branches of an `if`/
/// `unless`, or just the final `else` of a `case` -- an `elsif`/`when`
/// branch always gets the plain, unchecked replacement, matching
/// upstream's own asymmetry), and -- when `Layout/EndAlignment` wants `end`
/// aligned with the keyword -- pads before the terminal `end` with
/// `lhs_text.len()` spaces.
fn build_outer_correction(
    shape: Shape<'_>,
    tails: &[Node<'_>],
    lhs_text: &[u8],
    end_align_with_keyword: bool,
    ctx: &Context<'_>,
) -> Vec<Edit> {
    let mut edits = vec![Edit::insert(shape.span().start, lhs_text.to_vec())];
    let last_index = tails.len() - 1;
    let bracket_checked_case = matches!(shape, Shape::Case(_) | Shape::CaseMatch(_));
    for (i, tail) in tails.iter().enumerate() {
        let bracket_checked =
            if bracket_checked_case { i == last_index } else { i == 0 || i == last_index };
        let replacement = if bracket_checked {
            tail_rhs_replacement(tail, ctx)
        } else {
            plain_rhs_replacement(tail, ctx)
        };
        edits.push(Edit::replace(tail.span(), replacement));
    }
    if end_align_with_keyword {
        if let Some(end_span) = shape.end_keyword() {
            edits.push(Edit::insert(end_span.start, vec![b' '; lhs_text.len()]));
        }
    }
    edits
}

/// One `\s*`-separated literal run of RuboCop's `longest_line`
/// `assignment_regex` (`/\s*#{Regexp.escape(assignment).gsub('\ ', '\s*')}/`),
/// matched at `start`. Every run begins with a non-whitespace byte, so the
/// greedy `\s*` between runs never needs to backtrack.
fn match_assignment_at(
    line: &[u8],
    start: usize,
    segments: &[&[u8]],
    trailing_ws: bool,
) -> Option<usize> {
    fn skip_ws(line: &[u8], mut pos: usize) -> usize {
        while line.get(pos).is_some_and(u8::is_ascii_whitespace) {
            pos += 1;
        }
        pos
    }
    let mut pos = skip_ws(line, start);
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            pos = skip_ws(line, pos);
        }
        if !line[pos..].starts_with(segment) {
            return None;
        }
        pos += segment.len();
    }
    Some(if trailing_ws { skip_ws(line, pos) } else { pos })
}

fn char_len(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&b| (b & 0xC0) != 0x80).count()
}

/// RuboCop's `correction_exceeds_line_limit?`: the longest line of the
/// conditional, with the first occurrence of the reconstructed assignment
/// removed, plus the assignment itself, must fit `Layout/LineLength`'s
/// `Max`.
fn correction_exceeds_line_limit(
    node_span: Span,
    assignment: &[u8],
    max: i64,
    ctx: &Context<'_>,
) -> bool {
    let segments: Vec<&[u8]> =
        assignment.split(|&b| b == b' ').filter(|segment| !segment.is_empty()).collect();
    let trailing_ws = assignment.last() == Some(&b' ');
    let mut longest = 0usize;
    for line in ctx.text(node_span).split(|&b| b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let mut length = char_len(line);
        for start in 0..=line.len() {
            if let Some(end) = match_assignment_at(line, start, &segments, trailing_ws) {
                length = char_len(&line[..start]) + char_len(&line[end..]);
                break;
            }
        }
        longest = longest.max(length);
    }
    i64::try_from(char_len(assignment) + longest).is_ok_and(|length| length > max)
}

/// RuboCop's `on_if`/`on_case`/`on_case_match` plus `check_node`'s
/// `allowed_statements?` (`lhs_all_match?` + `statements.none?(&:masgn_type?)`
/// \+ `assignment_types_match?`), `allowed_single_line?`,
/// `correction_exceeds_line_limit?`, `safe_to_correct?`, and
/// `IfCorrector`/`CaseCorrector`/`TernaryCorrector`: the `assign_to_condition`
/// style (RuboCop's actual default).
fn check_outer(shape: Shape<'_>, rule: &ConditionalAssignment, ctx: &mut Context<'_>) {
    if rule.style != Style::AssignToCondition {
        return;
    }
    if shape.is_ternary() && !rule.include_ternary_expressions {
        return;
    }

    let branches = match shape {
        Shape::If(n) => match if_branch_bodies(&n) {
            Some(branches) => branches,
            None => return,
        },
        Shape::Unless(n) => unless_branch_bodies(&n),
        Shape::Case(n) => case_branch_bodies(&n.conditions(), n.else_clause()),
        Shape::CaseMatch(n) => case_branch_bodies(&n.conditions(), n.else_clause()),
    };
    if !shape.has_continuation() || branches.iter().any(Option::is_none) {
        return;
    }
    let mut tails: Vec<Node<'_>> = Vec::with_capacity(branches.len());
    for branch in branches.iter().flatten() {
        match branch.body().last() {
            Some(tail) => tails.push(tail),
            None => return,
        }
    }

    let Some(&first) = tails.first() else { return };
    if first.kind() == NodeKind::MultiWriteNode || rhs_of(&first).is_none() {
        return;
    }
    let first_kind = first.kind();
    let first_lhs = lhs(&first, ctx);
    for tail in &tails[1..] {
        if tail.kind() != first_kind || rhs_of(tail).is_none() || lhs(tail, ctx) != first_lhs {
            return;
        }
    }

    if rule.single_line_conditions_only
        && branches.iter().flatten().any(|branch| branch.body().len() > 1)
    {
        return;
    }
    if let Some(max) = rule.max_line_length {
        if correction_exceeds_line_limit(shape.span(), &first_lhs, max, ctx) {
            return;
        }
    }

    let edits = if shape.is_ternary() {
        let Shape::If(if_node) = shape else { return };
        let Some((if_tail, else_tail)) = ternary_parts(&if_node) else { return };
        let (Some(if_rhs), Some(else_rhs)) = (rhs_of(&if_tail), rhs_of(&else_tail)) else { return };
        let wrap = if_tail.kind() == NodeKind::CallNode
            && if_tail.as_call_node().is_some_and(|c| c.name().as_slice() != b"[]=");
        let mut replacement = first_lhs;
        if wrap {
            replacement.push(b'(');
        }
        replacement.extend_from_slice(ctx.text(if_node.predicate().span()));
        replacement.extend_from_slice(b" ? ");
        replacement.extend_from_slice(ctx.text(if_rhs.span()));
        replacement.extend_from_slice(b" : ");
        replacement.extend_from_slice(ctx.text(else_rhs.span()));
        if wrap {
            replacement.push(b')');
        }
        vec![Edit::replace(shape.span(), replacement)]
    } else {
        build_outer_correction(shape, &tails, &first_lhs, rule.end_align_with_keyword, ctx)
    };

    report_checked(shape.span(), MSG, edits, ctx);
}

impl Rule for ConditionalAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Style/ConditionalAssignment",
        department: Department::Style,
        summary: "Use the return value of `if` and `case` statements for assignment to a variable and variable comparison instead of assigning that variable inside of each branch.",
        explanation: "\
Checks for `if` and `case` statements where each branch is used for both the
assignment and comparison of the same variable when using the return of the
condition can be used instead.

```ruby
# EnforcedStyle: assign_to_condition (default)
# bad
if foo
  bar = 1
else
  bar = 2
end

# good
bar = if foo
        1
      else
        2
      end

# EnforcedStyle: assign_inside_condition
# bad
bar = if foo
        1
      else
        2
      end

# good
if foo
  bar = 1
else
  bar = 2
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::IfNode,
            NodeKind::UnlessNode,
            NodeKind::CaseNode,
            NodeKind::CaseMatchNode,
            NodeKind::LocalVariableWriteNode,
            NodeKind::InstanceVariableWriteNode,
            NodeKind::ClassVariableWriteNode,
            NodeKind::GlobalVariableWriteNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantPathWriteNode,
            NodeKind::MultiWriteNode,
            NodeKind::LocalVariableOperatorWriteNode,
            NodeKind::LocalVariableAndWriteNode,
            NodeKind::LocalVariableOrWriteNode,
            NodeKind::InstanceVariableOperatorWriteNode,
            NodeKind::InstanceVariableAndWriteNode,
            NodeKind::InstanceVariableOrWriteNode,
            NodeKind::ClassVariableOperatorWriteNode,
            NodeKind::ClassVariableAndWriteNode,
            NodeKind::ClassVariableOrWriteNode,
            NodeKind::GlobalVariableOperatorWriteNode,
            NodeKind::GlobalVariableAndWriteNode,
            NodeKind::GlobalVariableOrWriteNode,
            NodeKind::ConstantOperatorWriteNode,
            NodeKind::ConstantAndWriteNode,
            NodeKind::ConstantOrWriteNode,
            NodeKind::ConstantPathOperatorWriteNode,
            NodeKind::ConstantPathAndWriteNode,
            NodeKind::ConstantPathOrWriteNode,
            NodeKind::IndexOperatorWriteNode,
            NodeKind::IndexAndWriteNode,
            NodeKind::IndexOrWriteNode,
            NodeKind::CallOperatorWriteNode,
            NodeKind::CallAndWriteNode,
            NodeKind::CallOrWriteNode,
            NodeKind::CallNode,
        ],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("assign_to_condition"),
                allowed: &["assign_to_condition", "assign_inside_condition"],
                doc: "Whether to assign inside or outside of each conditional branch.",
            },
            ConfigOption {
                name: "SingleLineConditionsOnly",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Only register an offense when every branch is a single statement (assign_to_condition) or when no branch is multi-statement (assign_inside_condition).",
            },
            ConfigOption {
                name: "IncludeTernaryExpressions",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether ternary expressions should be considered by this cop.",
            },
        ],
        blind_spots: "\
`assign_to_condition`'s `lhs`'s `assignment_node.source` for an `op_asgn`/
`and_asgn`/`or_asgn` target is read off the target's own span/`name_loc`
rather than reconstructed textually; the two differ only when the original
source itself has unusual internal spacing inside a namespaced constant
path or an index's argument list, which no fixture exercises.
`assignment_rhs_exist?`'s `mlhs`/`resbody` guard is not needed: those
shapes never reach a Prism write-node kind this cop subscribes to in the
first place (a `for`-loop index is a bare target node, and a `rescue =>`
capture is too), so no offense is ever considered for them.
`safe_to_correct?` is ported as a real reparse of the corrected buffer, but
the `Parser::Source::TreeRewriter` clobbering errors it also rescues are
approximated by `merge_edits`: crossing removals merge into their union and
an insertion strictly inside a removal aborts, which is the only clobbering
shape either corrector can produce.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "assign_inside_condition" => Style::AssignInsideCondition,
            _ => Style::AssignToCondition,
        };
        let end_align_with_keyword = options
            .peer("Layout/EndAlignment", "EnforcedStyleAlignWith")
            .and_then(linter::OptionValue::as_str)
            .is_none_or(|s| s == "keyword");
        let max_line_length = options
            .peer("Layout/LineLength", "Enabled")
            .and_then(linter::OptionValue::as_bool)
            .unwrap_or(true)
            .then(|| {
                options
                    .peer("Layout/LineLength", "Max")
                    .and_then(linter::OptionValue::as_int)
                    .unwrap_or(120)
            });
        Ok(Self {
            style,
            single_line_conditions_only: options.bool("SingleLineConditionsOnly"),
            include_ternary_expressions: options.bool("IncludeTernaryExpressions"),
            end_align_with_keyword,
            max_line_length,
            ignored: Vec::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.ignored.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::IfNode => {
                let if_node = node.as_if_node().expect("kind matched");
                if !is_elsif(&if_node) {
                    check_outer(Shape::If(if_node), self, ctx);
                }
            }
            NodeKind::UnlessNode => {
                check_outer(Shape::Unless(node.as_unless_node().expect("kind matched")), self, ctx);
            }
            NodeKind::CaseNode => {
                check_outer(Shape::Case(node.as_case_node().expect("kind matched")), self, ctx);
            }
            NodeKind::CaseMatchNode => {
                check_outer(
                    Shape::CaseMatch(node.as_case_match_node().expect("kind matched")),
                    self,
                    ctx,
                );
            }
            _ => check_assignment_inside(
                node,
                self.style,
                self.include_ternary_expressions,
                self.single_line_conditions_only,
                &mut self.ignored,
                ctx,
            ),
        }
    }
}
