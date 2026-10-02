//! Ports of RuboCop's `Metrics::Utils` calculators: `CodeLengthCalculator`,
//! the `MethodComplexity` mixin's cyclomatic/perceived scoring, and
//! `AbcSizeCalculator` (with `RepeatedCsendDiscount`, `IteratingBlock` and
//! `RepeatedAttributeDiscount`).
//!
//! The upstream algorithms are written against whitequark's parser AST, which
//! differs from Prism's in three ways that matter here:
//!
//! * Prism has no `op_asgn`/`or_asgn`/`and_asgn` wrapper around a target
//!   node. `x ||= 1` is a single `LocalVariableOrWriteNode`, so one Prism node
//!   plays the role of *two* whitequark nodes (`or_asgn` plus its `lvasgn`
//!   child), and `foo.bar ||= 1` hides a whole `send` node that upstream
//!   counts as a branch. Both are re-synthesised below.
//! * Prism interposes container nodes (`ArgumentsNode`, `ParametersNode`,
//!   `ElseNode`) that whitequark does not have. They are transparent for
//!   parent lookups.
//! * Prism has no keyword-text accessors, so `if`/`elsif`/ternary and
//!   `else`/`:` are told apart by keyword-location *length* (2/5/absent and
//!   4/1), which is exact for these keywords.

use linter::{Context, RuleMeta, RuleOptions};
use regex::Regex;
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const, is_heredoc};
use ruby_ast::node::{CallNode, CaseMatchNode, CaseNode, ElseNode, IfNode, InNode};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{is_comment_line, SourceFile, Span};

// ---------------------------------------------------------------------------
// CodeLengthCalculator
// ---------------------------------------------------------------------------

/// One entry of a cop's `CountAsOne` option: a node shape that collapses to a
/// single line. Upstream's `CodeLengthCalculator::FOLDABLE_TYPES`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Foldable {
    /// `array`
    Array,
    /// `hash`, braced or not
    Hash,
    /// any heredoc string literal
    Heredoc,
    /// `send`/`csend`
    MethodCall,
}

impl Foldable {
    /// Parses a cop's `CountAsOne` list. Upstream raises a `Warning` for an
    /// unrecognised entry; a linter has nowhere to raise to during a
    /// traversal, so unknown entries are dropped.
    pub(crate) fn from_config(values: &[String]) -> Vec<Self> {
        values
            .iter()
            .filter_map(|value| match value.as_str() {
                "array" => Some(Self::Array),
                "hash" => Some(Self::Hash),
                "heredoc" => Some(Self::Heredoc),
                "method_call" => Some(Self::MethodCall),
                _ => None,
            })
            .collect()
    }

    /// Upstream's `build_foldable_checks`: does `node` actually fold?
    fn folds(self, node: &Node<'_>) -> bool {
        match self {
            Self::Array => matches!(node.kind(), NodeKind::ArrayNode),
            Self::Hash => matches!(node.kind(), NodeKind::HashNode | NodeKind::KeywordHashNode),
            Self::Heredoc => is_heredoc(node),
            Self::MethodCall => matches!(node.kind(), NodeKind::CallNode),
        }
    }

    /// Upstream's `normalize_foldable_types`: which node types the top-level
    /// descendant search stops at. Wider than [`Foldable::folds`] -- a `str`
    /// that is not a heredoc still halts the search, and is then not folded.
    fn halts(self, node: &Node<'_>) -> bool {
        match self {
            Self::Array => matches!(node.kind(), NodeKind::ArrayNode),
            Self::Hash => matches!(node.kind(), NodeKind::HashNode | NodeKind::KeywordHashNode),
            Self::Heredoc => {
                matches!(node.kind(), NodeKind::StringNode | NodeKind::InterpolatedStringNode)
            }
            Self::MethodCall => matches!(node.kind(), NodeKind::CallNode),
        }
    }
}

/// Port of `RuboCop::Cop::Metrics::Utils::CodeLengthCalculator#calculate`:
/// the number of relevant lines in `node`'s body, with every node matching
/// `foldable` collapsed to one line.
pub(crate) fn code_length(
    ctx: &Context<'_>,
    node: &Node<'_>,
    count_comments: bool,
    foldable: &[Foldable],
) -> u32 {
    code_length_in(ctx.source(), node, count_comments, foldable)
}

/// Upstream `CodeLength`'s `MSG`: `'%<label>s has too many lines. [%<length>d/%<max>d]'`.
pub(crate) fn message(label: &str, length: u32, max: i64) -> String {
    format!("{label} has too many lines. [{length}/{max}]")
}

/// The `CodeLength` mixin's configuration (`Max`, `CountComments`,
/// `CountAsOne`), shared by the four `*Length` cops.
#[derive(Debug, Clone)]
pub(crate) struct CodeLength {
    max: i64,
    count_comments: bool,
    foldable: Vec<Foldable>,
}

impl CodeLength {
    pub(crate) fn from_options(options: &RuleOptions) -> Self {
        Self {
            max: options.int("Max"),
            count_comments: options.bool("CountComments"),
            foldable: Foldable::from_config(&options.str_list("CountAsOne")),
        }
    }

    /// Port of `CodeLength#check_code_length`, minus the perf-only
    /// `line_count` short-circuit (upstream skips the full calculation when
    /// the node's raw physical-line count is already within `max`; the result
    /// is identical either way) and the `self.max = length` `exclude_limit`
    /// bookkeeping (only consumed by `--auto-gen-config`, which this linter
    /// does not implement).
    pub(crate) fn check(
        &self,
        ctx: &mut Context<'_>,
        meta: &RuleMeta,
        node: &Node<'_>,
        report_span: Span,
        label: &str,
    ) {
        let length = code_length(ctx, node, self.count_comments, &self.foldable);
        if i64::from(length) > self.max {
            ctx.report(meta, report_span, message(label, length, self.max));
        }
    }
}

/// `rubocop-ast`'s `Node#class_definition?`/`Node#module_definition?`,
/// restricted to their `any_block` alternative (the `class`/`module`/
/// `sclass` keyword-form alternatives are handled directly by
/// `ClassLength`/`ModuleLength` on the keyword nodes themselves): an
/// attached-block call to `.new` on one of `allowed`'s bare or
/// top-level-qualified receiver constants, e.g. `Struct.new(...) do ... end`
/// or `::Class.new do ... end`.
pub(crate) fn constructor_call<'pr>(value: &Node<'pr>, allowed: &[&str]) -> Option<CallNode<'pr>> {
    let call = value.as_call_node()?;
    call.block()?.as_block_node()?;
    if call.name().as_slice() != b"new" {
        return None;
    }
    let receiver = call.receiver()?;
    if !is_bare_or_toplevel_const(&receiver) {
        return None;
    }
    let name = const_name(&receiver)?;
    allowed.contains(&name.as_str()).then_some(call)
}

/// `rubocop-ast`'s `Node#assignment?` restricted to constant targets, fused
/// with the RHS extraction `ClassLength`/`ModuleLength`'s `on_casgn` does by
/// hand (`node.expression || find_expression_within_parent(node.parent)`):
/// the assigned value of a plain (`FOO = v`), compound (`FOO ||= v`,
/// `FOO &&= v`, `FOO op= v`, and their `A::FOO` path equivalents), or
/// multiple (`FOO, BAR = v`) assignment to a constant. Prism gives each of
/// these its own flat node carrying `.value()` directly, unlike whitequark,
/// where a compound assignment nests a valueless `casgn` inside an
/// `or_asgn`/`and_asgn`/`op_asgn`, and a multiple assignment nests one
/// valueless `casgn` per target inside an `mlhs` inside a `masgn` -- both
/// reached upstream by walking `casgn`'s own `parent`/`parent.parent`.
pub(crate) fn assigned_value<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    match node.kind() {
        NodeKind::ConstantWriteNode => Some(node.as_constant_write_node()?.value()),
        NodeKind::ConstantPathWriteNode => Some(node.as_constant_path_write_node()?.value()),
        NodeKind::ConstantOrWriteNode => Some(node.as_constant_or_write_node()?.value()),
        NodeKind::ConstantAndWriteNode => Some(node.as_constant_and_write_node()?.value()),
        NodeKind::ConstantOperatorWriteNode => {
            Some(node.as_constant_operator_write_node()?.value())
        }
        NodeKind::ConstantPathOrWriteNode => Some(node.as_constant_path_or_write_node()?.value()),
        NodeKind::ConstantPathAndWriteNode => Some(node.as_constant_path_and_write_node()?.value()),
        NodeKind::ConstantPathOperatorWriteNode => {
            Some(node.as_constant_path_operator_write_node()?.value())
        }
        NodeKind::MultiWriteNode => {
            let multi = node.as_multi_write_node()?;
            let has_const_target = multi.lefts().iter().any(|n| is_constant_target(&n))
                || multi.rights().iter().any(|n| is_constant_target(&n));
            has_const_target.then(|| multi.value())
        }
        _ => None,
    }
}

fn is_constant_target(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::ConstantTargetNode | NodeKind::ConstantPathTargetNode)
}

fn code_length_in(
    src: &SourceFile,
    node: &Node<'_>,
    count_comments: bool,
    foldable: &[Foldable],
) -> u32 {
    let mut length = i64::from(node_length(src, node, count_comments));
    if foldable.is_empty() {
        return u32::try_from(length).unwrap_or(0);
    }

    each_top_level_descendant(node, None, foldable, &mut |descendant, parent| {
        if !foldable.iter().any(|f| f.folds(descendant)) {
            return;
        }
        length = length - i64::from(node_length(src, descendant, count_comments)) + 1;
        if matches!(descendant.kind(), NodeKind::KeywordHashNode) {
            length -= i64::from(omit_length(descendant, parent.as_ref()));
        }
    });

    u32::try_from(length).unwrap_or(0)
}

/// Upstream's private `code_length(node)`.
fn node_length(src: &SourceFile, node: &Node<'_>, count_comments: bool) -> u32 {
    if is_classlike(node) {
        return classlike_length(src, node, count_comments);
    }
    if is_heredoc(node) {
        return heredoc_length(src, node, count_comments);
    }
    let Some(body) = extract_body(node) else { return 0 };
    let span = body_span(&body);

    if let Some(last_line) = heredoc_extended_last_line(src, &body) {
        let first_line = src.line_col(span.start).line;
        (first_line..=last_line).filter(|&l| !irrelevant(src.line_text(l), count_comments)).count()
    } else {
        src.slice(span)
            .split_inclusive(|&b| b == b'\n')
            .filter(|line| !irrelevant(line, count_comments))
            .count()
    }
    .try_into()
    .unwrap_or(u32::MAX)
}

/// The byte range whitequark would give a body node.
///
/// Prism gives an *implicit* `BeginNode` -- the one standing for a
/// `def`/`block`/`class` body that carries `rescue`/`else`/`ensure` clauses --
/// the span of the whole enclosing construct, keywords included. whitequark's
/// equivalent `rescue`/`ensure` node covers only the clauses themselves, so
/// the span is rebuilt from the clause parts.
fn body_span(node: &Node<'_>) -> ruby_source::Span {
    let Some(begin) = node.as_begin_node() else { return node.span() };
    if begin.begin_keyword_loc().is_some() {
        return node.span();
    }

    let statements = begin.statements().map(|s| s.as_node().span());
    let rescue = begin.rescue_clause().map(|r| r.as_node().span());
    let start = statements.or(rescue).map_or_else(|| node.span().start, |span| span.start);

    let mut end = statements.map_or(start, |span| span.end);
    if let Some(rescue) = rescue {
        end = end.max(rescue.end);
    }
    if let Some(else_clause) = begin.else_clause() {
        end =
            end.max(else_clause.statements().map_or_else(
                || else_clause.else_keyword_loc().span().end,
                |s| s.as_node().span().end,
            ));
    }
    if let Some(ensure) = begin.ensure_clause() {
        end = end.max(
            ensure
                .statements()
                .map_or_else(|| ensure.ensure_keyword_loc().span().end, |s| s.as_node().span().end),
        );
    }
    ruby_source::Span::new(start, end)
}

/// Upstream's `classlike_code_length`. The `line_number + 1` lookup is not a
/// typo: upstream indexes the 0-based `ProcessedSource#[]` line array with
/// 1-based line numbers from `line_range`, so every line it inspects is the
/// one *after* the line it excluded. Reproduced so counts match.
fn classlike_length(src: &SourceFile, node: &Node<'_>, count_comments: bool) -> u32 {
    let body = extract_body(node);
    if body.as_ref().is_some_and(is_classlike) {
        return 0;
    }

    let span = node.span();
    let first = src.line_col(span.start).line;
    let last = src.last_line(span);
    if last <= first + 1 {
        return 0;
    }

    let mut count = 0;
    for line in (first + 1)..last {
        if inner_classlike_covers(src, node, line) {
            continue;
        }
        if !irrelevant(src.line_text(line + 1), count_comments) {
            count += 1;
        }
    }
    count
}

/// Upstream's `line_numbers_of_inner_nodes(node, :module, :class)`, asked one
/// line at a time so no line-number set has to be materialised.
fn inner_classlike_covers(src: &SourceFile, node: &Node<'_>, line: u32) -> bool {
    let mut covered = false;
    each_descendant(node, &mut |descendant| {
        if covered || !is_classlike(descendant) {
            return;
        }
        let span = descendant.span();
        covered = src.line_col(span.start).line <= line && line <= src.last_line(span);
    });
    covered
}

/// Upstream's `heredoc_length`: the heredoc body's relevant lines, plus the
/// opening and closing lines.
fn heredoc_length(src: &SourceFile, node: &Node<'_>, count_comments: bool) -> u32 {
    let Some((first, last)) = heredoc_body_lines(src, node) else { return 2 };
    let body: u32 = (first..=last)
        .filter(|&l| !irrelevant(src.line_text(l), count_comments))
        .count()
        .try_into()
        .unwrap_or(u32::MAX);
    body + 2
}

/// First and last 1-based line of a heredoc's body, or `None` when it is
/// empty. Upstream reads `node.loc.heredoc_body`, which Prism does not model;
/// the body is everything between the content start and the terminator line.
fn heredoc_body_lines(src: &SourceFile, node: &Node<'_>) -> Option<(u32, u32)> {
    let content_start = if let Some(string) = node.as_string_node() {
        string.content_loc().span().start
    } else if let Some(string) = node.as_x_string_node() {
        string.content_loc().span().start
    } else if let Some(string) = node.as_interpolated_string_node() {
        string.parts().iter().next()?.span().start
    } else {
        let string = node.as_interpolated_x_string_node()?;
        string.parts().iter().next()?.span().start
    };
    let closing = heredoc_closing_loc(node)?.span().start;
    let first = src.line_col(content_start).line;
    let last = src.line_col(closing).line.checked_sub(1)?;
    (first <= last).then_some((first, last))
}

/// Upstream's `node_with_heredoc?` + `source_from_node_with_heredoc`, fused:
/// the last line the body's source really occupies once heredoc bodies are
/// taken into account, or `None` when the body contains no heredoc.
fn heredoc_extended_last_line(src: &SourceFile, body: &Node<'_>) -> Option<u32> {
    let mut has_heredoc = false;
    each_descendant(body, &mut |descendant| {
        if !has_heredoc
            && matches!(descendant.kind(), NodeKind::StringNode | NodeKind::InterpolatedStringNode)
            && is_heredoc(descendant)
        {
            has_heredoc = true;
        }
    });
    if !has_heredoc {
        return None;
    }

    let mut last = 0;
    each_descendant(body, &mut |descendant| {
        let line = if is_heredoc(descendant) {
            heredoc_closing_line(src, descendant)
        } else {
            src.last_line(descendant.span())
        };
        last = last.max(line);
    });
    Some(last)
}

/// The terminator location of a heredoc literal, whatever its flavour.
fn heredoc_closing_loc<'pr>(node: &Node<'pr>) -> Option<ruby_ast::Location<'pr>> {
    if let Some(string) = node.as_string_node() {
        string.closing_loc()
    } else if let Some(string) = node.as_interpolated_string_node() {
        string.closing_loc()
    } else if let Some(string) = node.as_x_string_node() {
        Some(string.closing_loc())
    } else {
        node.as_interpolated_x_string_node().map(|string| string.closing_loc())
    }
}

fn heredoc_closing_line(src: &SourceFile, node: &Node<'_>) -> u32 {
    heredoc_closing_loc(node)
        .map_or_else(|| src.last_line(node.span()), |loc| src.line_col(loc.span().start).line)
}

/// Upstream's `extract_body`.
///
/// Prism always wraps a body in a `StatementsNode`; whitequark only has an
/// equivalent (`begin`) when there is more than one statement, so a lone
/// statement is unwrapped. That matters for `namespace_module?`, which asks
/// whether the body *is* a class or module.
///
/// The `CallNode`/`SuperNode`/`ForwardingSuperNode` arms are Prism's stand-in
/// for whitequark's `:block` case: whitequark wraps a call plus its literal
/// block in one `block`-type node, whose own `.body` is the statements
/// inside; Prism instead attaches the block to the call/`super` as a field,
/// so a `Struct.new(...) do ... end`/`Class.new do ... end`/`Module.new do
/// ... end` value (what `ClassLength`/`ModuleLength`'s constant-assignment
/// handling passes here), or any other attached-block call/`super`
/// (`Metrics/BlockLength`), is a bare `CallNode`/`SuperNode`/
/// `ForwardingSuperNode`, and its body must be reached through the attached
/// block explicitly. `LambdaNode` (`->() { ... }`/`->() do ... end`) is its
/// own node kind in Prism, unlike whitequark's `(block (send nil :lambda)
/// ...)`, but already exposes its statements directly via `.body`.
fn extract_body<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    let body = match node.kind() {
        NodeKind::ClassNode => node.as_class_node()?.body(),
        NodeKind::ModuleNode => node.as_module_node()?.body(),
        NodeKind::SingletonClassNode => node.as_singleton_class_node()?.body(),
        NodeKind::BlockNode => node.as_block_node()?.body(),
        NodeKind::DefNode => node.as_def_node()?.body(),
        NodeKind::CallNode => {
            let call = node.as_call_node()?;
            call.block().and_then(|b| b.as_block_node()?.body()).or(Some(*node))
        }
        NodeKind::SuperNode => {
            let s = node.as_super_node()?;
            s.block().and_then(|b| b.as_block_node()?.body()).or(Some(*node))
        }
        NodeKind::ForwardingSuperNode => {
            let s = node.as_forwarding_super_node()?;
            s.block().and_then(|b| b.body()).or(Some(*node))
        }
        NodeKind::LambdaNode => node.as_lambda_node()?.body(),
        _ => Some(*node),
    }?;
    Some(unwrap_single_statement(&body))
}

fn unwrap_single_statement<'pr>(node: &Node<'pr>) -> Node<'pr> {
    let Some(statements) = node.as_statements_node() else { return *node };
    let mut body = statements.body().iter();
    match (body.next(), body.next()) {
        (Some(only), None) => only,
        _ => *node,
    }
}

/// Upstream's `irrelevant_line?`.
fn irrelevant(line: &[u8], count_comments: bool) -> bool {
    line.iter().all(u8::is_ascii_whitespace) || (!count_comments && is_comment_line(line))
}

fn is_classlike(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::ClassNode | NodeKind::ModuleNode)
}

/// Upstream's `each_top_level_descendant`. `inherited` is the whitequark
/// parent to report for `node`'s children; Prism's `ArgumentsNode` has no
/// whitequark counterpart and passes its own parent through.
fn each_top_level_descendant<'pr>(
    node: &Node<'pr>,
    inherited: Option<Node<'pr>>,
    foldable: &[Foldable],
    f: &mut impl FnMut(&Node<'pr>, Option<Node<'pr>>),
) {
    let child_parent =
        if matches!(node.kind(), NodeKind::ArgumentsNode) { inherited } else { Some(*node) };
    for_each_child(node, |child| {
        if is_classlike(child) {
            return;
        }
        if foldable.iter().any(|t| t.halts(child)) {
            f(child, child_parent);
        } else {
            each_top_level_descendant(child, child_parent, foldable, f);
        }
    });
}

/// Upstream's `omit_length`: a braceless hash argument still costs the
/// parentheses-adjacent characters its braces would have occupied.
fn omit_length(descendant: &Node<'_>, parent: Option<&Node<'_>>) -> u32 {
    let Some(call) = parent.and_then(Node::as_call_node) else { return 0 };
    if call.arguments().is_some_and(|args| args.arguments().iter().count() > 1) {
        return 0;
    }
    let (Some(opening), Some(closing)) = (call.opening_loc(), call.closing_loc()) else {
        return 0;
    };
    let span = descendant.span();
    u32::from(opening.span().end != span.start) + u32::from(closing.span().start != span.end)
}

/// `for_each_child` applied recursively -- `ruby_ast::each_descendant` with a
/// `&mut` callback that can stop contributing early.
///
/// A call's attached block is transparent here: `f` is never invoked on the
/// `BlockNode` wrapper itself (only recursed into, for its parameters and
/// statements), since whitequark has no node standing for just the wrapper
/// -- a call plus its literal block is *one* `:block`-type node there, whose
/// own boundary (through the closing `end`/`}`) is attributed to the call
/// itself (a Prism `CallNode`'s span already extends through its attached
/// block, matching a whitequark `:block` node's `.last_line`). Visiting the
/// wrapper too would double-count that same boundary a second time -- e.g.
/// `source_from_node_with_heredoc`'s "last line among all descendants" scan
/// (ported as `heredoc_extended_last_line`) would otherwise extend a
/// statement's measured length through a nested block's own closing `end`,
/// which whitequark's fused node never contributes as a distinct child.
///
/// An `ElseNode` (an `if`/`unless`/`case`'s `else` clause), an
/// `EnsureNode`, and an `elsif` link (an `IfNode` reused for the purpose,
/// per `is_elsif`) all get the same treatment and for the same reason:
/// whitequark has no node standing for just the `else`/`ensure`/`elsif`
/// clause -- the enclosing `if`/`unless`/`case`/`begin` node's own child
/// *is* that branch's body directly -- but Prism wraps each in its own
/// node, whose span reaches through the enclosing `end` too (confirmed
/// empirically for `EnsureNode`; `elsif` links are documented to "share the
/// `end_keyword_loc`" with the `if` they belong to). `body_span` already
/// sidesteps this for the *outermost* rescue/ensure by reading
/// `.statements()` instead of the clause node's own span; this walk needs
/// the identical fix for one reached as an ordinary, deeper descendant.
fn each_descendant<'pr>(node: &Node<'pr>, f: &mut impl FnMut(&Node<'pr>)) {
    for_each_child(node, |child| {
        let is_transparent = child.as_block_node().is_some()
            || child.as_else_node().is_some()
            || child.as_ensure_node().is_some()
            || child.as_if_node().is_some_and(|i| is_elsif(&i));
        if is_transparent {
            each_descendant(child, f);
        } else {
            f(child);
            each_descendant(child, f);
        }
    });
}

// ---------------------------------------------------------------------------
// Shared node classification
// ---------------------------------------------------------------------------

/// `CyclomaticComplexity::COUNTED_NODES`, also `AbcSizeCalculator`'s
/// `CONDITION_NODES`: `if while until for csend block block_pass rescue when
/// in_pattern and or or_asgn and_asgn`.
///
/// `numblock` and `itblock` are *not* on that list, and whitequark gives a
/// block using `_1`/`it` one of those types rather than `block`, so such a
/// block never counts. `it_parameter` says whether the target Ruby version
/// really reads a bare `it` as the implicit parameter (3.4 and later); below
/// that, `it` is an ordinary method call and the block stays a `block`.
fn is_counted_cyclomatic(node: &Node<'_>, it_parameter: bool) -> bool {
    match node.kind() {
        NodeKind::IfNode
        | NodeKind::UnlessNode
        | NodeKind::ForNode
        | NodeKind::BlockArgumentNode
        | NodeKind::RescueModifierNode
        | NodeKind::WhenNode
        | NodeKind::InNode
        | NodeKind::AndNode
        | NodeKind::OrNode => true,
        // `begin ... end while cond` is whitequark's `while_post`, which is
        // not on the list; only a plain `while`/`until` is.
        NodeKind::WhileNode => {
            node.as_while_node().is_some_and(|loop_node| !loop_node.is_begin_modifier())
        }
        NodeKind::UntilNode => {
            node.as_until_node().is_some_and(|loop_node| !loop_node.is_begin_modifier())
        }
        NodeKind::BlockNode => match node.as_block_node().and_then(|block| block.parameters()) {
            Some(parameters) => match parameters.kind() {
                NodeKind::NumberedParametersNode => false,
                NodeKind::ItParametersNode => !it_parameter,
                _ => true,
            },
            None => true,
        },
        // Prism models `begin/rescue` as a `BeginNode` owning a `RescueNode`
        // chain; whitequark has a single `rescue` node for the construct.
        NodeKind::BeginNode => {
            node.as_begin_node().is_some_and(|begin| begin.rescue_clause().is_some())
        }
        NodeKind::CallNode => node.as_call_node().is_some_and(|call| call.is_safe_navigation()),
        kind => is_or_asgn(kind) || is_and_asgn(kind),
    }
}

/// Whether a bare `it` inside a parameterless block is the implicit block
/// parameter, which Ruby only made it in 3.4. Prism always parses it that
/// way; whitequark follows `TargetRubyVersion`.
pub(crate) fn it_is_parameter(target_ruby_version: f32) -> bool {
    target_ruby_version > 3.35
}

fn is_or_asgn(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableOrWriteNode
            | NodeKind::InstanceVariableOrWriteNode
            | NodeKind::ClassVariableOrWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::ConstantOrWriteNode
            | NodeKind::ConstantPathOrWriteNode
            | NodeKind::CallOrWriteNode
            | NodeKind::IndexOrWriteNode
    )
}

fn is_and_asgn(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableAndWriteNode
            | NodeKind::InstanceVariableAndWriteNode
            | NodeKind::ClassVariableAndWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::ConstantAndWriteNode
            | NodeKind::ConstantPathAndWriteNode
            | NodeKind::CallAndWriteNode
            | NodeKind::IndexAndWriteNode
    )
}

fn is_op_asgn(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableOperatorWriteNode
            | NodeKind::InstanceVariableOperatorWriteNode
            | NodeKind::ClassVariableOperatorWriteNode
            | NodeKind::GlobalVariableOperatorWriteNode
            | NodeKind::ConstantOperatorWriteNode
            | NodeKind::ConstantPathOperatorWriteNode
            | NodeKind::CallOperatorWriteNode
            | NodeKind::IndexOperatorWriteNode
    )
}

/// `Node#shorthand_asgn?`.
fn is_shorthand_asgn(kind: NodeKind) -> bool {
    is_or_asgn(kind) || is_and_asgn(kind) || is_op_asgn(kind)
}

/// Every Prism node that stands for a whitequark `lvasgn`: the plain write,
/// the multiple-assignment/`for`/`rescue =>` target, and the three
/// abbreviated-assignment forms that in whitequark wrap an `lvasgn` child.
///
/// Prism also uses `LocalVariableTargetNode` for pattern-match captures,
/// where whitequark emits `match_var`; the only consequence is that a capture
/// resets the repeated-`&.` chain for that name, which upstream would not.
fn lvasgn_name<'pr>(node: &Node<'pr>) -> Option<&'pr [u8]> {
    match node.kind() {
        NodeKind::LocalVariableWriteNode => {
            Some(node.as_local_variable_write_node()?.name().as_slice())
        }
        NodeKind::LocalVariableTargetNode => {
            Some(node.as_local_variable_target_node()?.name().as_slice())
        }
        NodeKind::LocalVariableOrWriteNode => {
            Some(node.as_local_variable_or_write_node()?.name().as_slice())
        }
        NodeKind::LocalVariableAndWriteNode => {
            Some(node.as_local_variable_and_write_node()?.name().as_slice())
        }
        NodeKind::LocalVariableOperatorWriteNode => {
            Some(node.as_local_variable_operator_write_node()?.name().as_slice())
        }
        _ => None,
    }
}

/// `IteratingBlock#block_method_name`: the method a `block`/`block_pass` node
/// belongs to. Prism attaches both to the call node directly.
///
/// whitequark's `SuperNode#method_name` is `:super` for both `super` and
/// `zsuper`, so `super { ... }` and `super(&blk)` report that name rather
/// than nothing.
fn block_method_name<'pr>(node: &Node<'pr>, parent: Option<&Node<'pr>>) -> Option<&'pr [u8]> {
    if !matches!(node.kind(), NodeKind::BlockNode | NodeKind::BlockArgumentNode) {
        return None;
    }
    let parent = parent?;
    if matches!(parent.kind(), NodeKind::SuperNode | NodeKind::ForwardingSuperNode) {
        return Some(b"super");
    }
    parent.as_call_node().map(|call| call.name().as_slice()).or_else(|| {
        parent
            .as_call_or_write_node()
            .map(|call| call.read_name().as_slice())
            .or_else(|| parent.as_call_and_write_node().map(|call| call.read_name().as_slice()))
            .or_else(|| {
                parent.as_call_operator_write_node().map(|call| call.read_name().as_slice())
            })
    })
}

/// `IteratingBlock#iterating_block?`: `None` when `node` is neither a block
/// nor a block-pass, otherwise whether the call iterates. Upstream returns
/// `nil` -- not `false` -- when the method name cannot be determined, and
/// every caller tests `== false`, so an unknown name must not read as "not
/// iterating".
fn iterating_block(node: &Node<'_>, parent: Option<&Node<'_>>) -> Option<bool> {
    if !matches!(node.kind(), NodeKind::BlockNode | NodeKind::BlockArgumentNode) {
        return None;
    }
    Some(is_iterating_method(block_method_name(node, parent)?))
}

/// `IteratingBlock::KNOWN_ITERATING_METHODS`.
fn is_iterating_method(name: &[u8]) -> bool {
    const ENUMERABLE: &[&[u8]] = &[
        b"all?",
        b"any?",
        b"chain",
        b"chunk",
        b"chunk_while",
        b"collect",
        b"collect_concat",
        b"count",
        b"cycle",
        b"detect",
        b"drop",
        b"drop_while",
        b"each",
        b"each_cons",
        b"each_entry",
        b"each_slice",
        b"each_with_index",
        b"each_with_object",
        b"entries",
        b"filter",
        b"filter_map",
        b"find",
        b"find_all",
        b"find_index",
        b"flat_map",
        b"grep",
        b"grep_v",
        b"group_by",
        b"inject",
        b"lazy",
        b"map",
        b"max",
        b"max_by",
        b"min",
        b"min_by",
        b"minmax",
        b"minmax_by",
        b"none?",
        b"one?",
        b"partition",
        b"reduce",
        b"reject",
        b"reverse_each",
        b"select",
        b"slice_after",
        b"slice_before",
        b"slice_when",
        b"sort",
        b"sort_by",
        b"sum",
        b"take",
        b"take_while",
        b"tally",
        b"to_h",
        b"uniq",
        b"zip",
    ];
    const ENUMERATOR: &[&[u8]] = &[b"with_index", b"with_object"];
    const ARRAY: &[&[u8]] = &[
        b"bsearch",
        b"bsearch_index",
        b"collect!",
        b"combination",
        b"d_permutation",
        b"delete_if",
        b"each_index",
        b"keep_if",
        b"map!",
        b"permutation",
        b"product",
        b"reject!",
        b"repeat",
        b"repeated_combination",
        b"select!",
        b"sort",
        b"sort!",
        b"sort_by",
    ];
    const HASH: &[&[u8]] = &[
        b"each_key",
        b"each_pair",
        b"each_value",
        b"fetch",
        b"fetch_values",
        b"has_key?",
        b"merge",
        b"merge!",
        b"transform_keys",
        b"transform_keys!",
        b"transform_values",
        b"transform_values!",
    ];
    [ENUMERABLE, ENUMERATOR, ARRAY, HASH].iter().any(|set| set.contains(&name))
}

/// `IfNode#elsif?`: Prism reuses `IfNode` for `elsif`, distinguished only by
/// the keyword, whose length (5 vs 2) identifies it.
fn is_elsif(node: &IfNode<'_>) -> bool {
    node.if_keyword_loc().is_some_and(|loc| loc.span().len() == 5)
}

/// `IfNode#else?`: `loc?(:else)`. True for an `elsif` chain (the `else`
/// location is then the `elsif` keyword) and false for a ternary, whose
/// `ElseNode` keyword is `:` rather than `else`.
fn if_has_else(node: &IfNode<'_>) -> bool {
    node.subsequent().is_some_and(|subsequent| {
        subsequent
            .as_else_node()
            .is_none_or(|else_node| else_node.else_keyword_loc().span().len() == 4)
    })
}

/// `AbcSizeCalculator#else_branch?`: the node has an `else` clause whose
/// keyword really is `else` (so a ternary's `:` does not count).
fn is_real_else(else_node: &ElseNode<'_>) -> bool {
    else_node.else_keyword_loc().span().len() == 4
}

/// Recursive pre-order walk carrying each node's Prism parent.
fn walk_with_parent<'pr>(
    node: &Node<'pr>,
    parent: Option<&Node<'pr>>,
    in_pattern: bool,
    f: &mut impl FnMut(&Node<'pr>, Option<&Node<'pr>>, bool),
) {
    f(node, parent, in_pattern);
    for_each_child(node, |child| {
        walk_with_parent(child, Some(node), child_in_pattern(node, child, in_pattern), f);
    });
}

/// Whether `child` sits in pattern-matching position, where Prism's
/// `LocalVariableTargetNode` stands for whitequark's `match_var` rather than
/// an `lvasgn`.
///
/// A branch guard (`in pat if cond`) is Prism's modifier `IfNode` wrapping
/// the pattern, so its condition is marked too; only an assignment written
/// inside a guard would notice, and that is not expressible without
/// parentheses.
fn child_in_pattern<'pr>(node: &Node<'pr>, child: &Node<'pr>, in_pattern: bool) -> bool {
    if in_pattern || is_pattern_node(node.kind()) {
        return true;
    }
    let pattern = match node.kind() {
        NodeKind::InNode => node.as_in_node().map(|in_node| in_node.pattern()),
        NodeKind::MatchRequiredNode => {
            node.as_match_required_node().map(|match_node| match_node.pattern())
        }
        NodeKind::MatchPredicateNode => {
            node.as_match_predicate_node().map(|match_node| match_node.pattern())
        }
        _ => None,
    };
    pattern.is_some_and(|pattern| pattern.span().start == child.span().start)
}

/// Node kinds that only ever occur inside a pattern, and so carry pattern
/// position down to everything below them.
fn is_pattern_node(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::ArrayPatternNode
            | NodeKind::FindPatternNode
            | NodeKind::HashPatternNode
            | NodeKind::CapturePatternNode
            | NodeKind::AlternationPatternNode
            | NodeKind::ImplicitNode
            | NodeKind::PinnedExpressionNode
            | NodeKind::PinnedVariableNode
    )
}

// ---------------------------------------------------------------------------
// RepeatedCsendDiscount
// ---------------------------------------------------------------------------

/// Port of `Metrics::Utils::RepeatedCsendDiscount`: the first `&.` call on a
/// given local variable is charged, later ones on the same variable are not,
/// until the variable is reassigned.
#[derive(Default)]
struct RepeatedCsend<'pr> {
    seen: Vec<(&'pr [u8], u32)>,
}

impl<'pr> RepeatedCsend<'pr> {
    /// `discount_for_repeated_csend?`. `span_start` stands in for upstream's
    /// node identity: every node in a file has a unique start offset.
    fn discount(&mut self, receiver: Option<&Node<'pr>>, span_start: u32) -> bool {
        let Some(read) = receiver.and_then(Node::as_local_variable_read_node) else {
            return false;
        };
        let name = read.name().as_slice();
        if let Some(&(_, start)) = self.seen.iter().find(|(seen, _)| *seen == name) {
            return start != span_start;
        }
        self.seen.push((name, span_start));
        false
    }

    fn reset(&mut self, name: &[u8]) {
        self.seen.retain(|(seen, _)| *seen != name);
    }
}

// ---------------------------------------------------------------------------
// MethodComplexity
// ---------------------------------------------------------------------------

/// Port of `Metrics::CyclomaticComplexity`'s scoring over
/// `MethodComplexity#complexity`. Call with a method or block *body*, as
/// upstream's `check_complexity` does.
pub(crate) fn cyclomatic(node: &Node<'_>, it_parameter: bool) -> u32 {
    complexity(node, false, it_parameter)
}

/// Port of `Metrics::PerceivedComplexity`'s scoring: `when` is replaced by
/// `case` (scored by branch count) and an `if` with a real `else` counts
/// double.
pub(crate) fn perceived(node: &Node<'_>, it_parameter: bool) -> u32 {
    complexity(node, true, it_parameter)
}

fn complexity(node: &Node<'_>, perceived: bool, it_parameter: bool) -> u32 {
    let mut score = 1;
    let mut csend = RepeatedCsend::default();
    walk_with_parent(node, None, false, &mut |current, parent, in_pattern| {
        score += complexity_score(current, parent, perceived, it_parameter, &mut csend);
        if in_pattern {
            return;
        }
        if let Some(name) = lvasgn_name(current) {
            csend.reset(name);
        }
    });
    score
}

fn complexity_score<'pr>(
    node: &Node<'pr>,
    parent: Option<&Node<'pr>>,
    perceived: bool,
    it_parameter: bool,
    csend: &mut RepeatedCsend<'pr>,
) -> u32 {
    let kind = node.kind();
    if perceived {
        // `COUNTED_NODES - [:when, :in_pattern] + [:case, :case_match]`.
        if matches!(kind, NodeKind::WhenNode | NodeKind::InNode) {
            return 0;
        }
        if let Some(case) = node.as_case_node() {
            return case_score(&case);
        }
        if let Some(case) = node.as_case_match_node() {
            return case_match_score(&case);
        }
    }
    if is_pattern_guard(node, parent) || !is_counted_cyclomatic(node, it_parameter) {
        return implicit_call_score(node, csend);
    }
    if perceived {
        if let Some(if_node) = node.as_if_node() {
            return if if_has_else(&if_node) && !is_elsif(&if_node) { 2 } else { 1 };
        }
        if let Some(unless_node) = node.as_unless_node() {
            return if unless_node.else_clause().is_some() { 2 } else { 1 };
        }
    }
    if iterating_block(node, parent) == Some(false) {
        return 0;
    }
    if let Some(call) = node.as_call_node() {
        if call.is_safe_navigation() && csend.discount(call.receiver().as_ref(), node.span().start)
        {
            return 0;
        }
    }
    1 + implicit_call_score(node, csend)
}

/// A `Call*WriteNode`/`Index*WriteNode` hides a whitequark `send`/`csend`
/// child that upstream walks as a node of its own; for cyclomatic scoring
/// only a `csend` counts.
fn implicit_call_score<'pr>(node: &Node<'pr>, csend: &mut RepeatedCsend<'pr>) -> u32 {
    let Some(call) = implicit_call(node) else { return 0 };
    if !call.safe_navigation {
        return 0;
    }
    u32::from(!csend.discount(call.receiver.as_ref(), node.span().start))
}

/// whitequark keeps a `case`/`in` branch guard in its own `if_guard`/
/// `unless_guard` node, which is not an `if` and so is never counted. Prism
/// instead wraps the branch's pattern in a modifier `IfNode`/`UnlessNode`
/// hanging straight off the `InNode`, which must not be counted either.
fn is_pattern_guard(node: &Node<'_>, parent: Option<&Node<'_>>) -> bool {
    matches!(node.kind(), NodeKind::IfNode | NodeKind::UnlessNode)
        && parent.is_some_and(|parent| parent.kind() == NodeKind::InNode)
}

/// Upstream's `PerceivedComplexity#complexity_score_for` for `case`: `0.8`
/// for the `case` plus `0.2` per branch, rounded -- computed in tenths, so
/// the result never depends on binary floating point. `nb_branches * 0.2`
/// can never land on a half, since `2 * nb_branches + 8` is always even.
fn case_score(case: &CaseNode<'_>) -> u32 {
    let branches = branch_count(case.conditions().iter().count(), case.else_clause().is_some());
    if case.predicate().is_none() {
        branches
    } else {
        (2 * branches + 13) / 10
    }
}

/// Upstream's `complexity_score_for` for `case_match`: a full point per
/// structural or guarded `in` branch, `0.2` for a branch whose pattern is a
/// plain literal or a constant, and `0.2` for an `else`. Again in tenths.
fn case_match_score(case: &CaseMatchNode<'_>) -> u32 {
    let mut tenths = 0;
    for condition in &case.conditions() {
        let simple = condition.as_in_node().is_some_and(|in_node| is_simple_in_pattern(&in_node));
        tenths += if simple { 2 } else { 10 };
    }
    if case.else_clause().is_some() {
        tenths += 2;
    }
    (tenths + 5) / 10
}

/// `nb_branches`: the `when`/`in` branches plus the `else`, saturating so no
/// cast is needed.
fn branch_count(conditions: usize, has_else: bool) -> u32 {
    let branches = conditions + usize::from(has_else);
    u32::try_from(branches).unwrap_or(u32::MAX)
}

/// Upstream's `simple_in_pattern?`: no guard, and a pattern that is a scalar
/// literal, a literal range or a constant.
fn is_simple_in_pattern(in_node: &InNode<'_>) -> bool {
    let pattern = in_node.pattern();
    if matches!(pattern.kind(), NodeKind::IfNode | NodeKind::UnlessNode) {
        return false;
    }
    is_pattern_literal(&pattern)
}

/// `Node#literal?` (`RuboCop::AST::Node::LITERALS`) or `Node#const_type?`,
/// restricted to what can appear in pattern position.
fn is_pattern_literal(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::ArrayNode
            | NodeKind::HashNode
            | NodeKind::KeywordHashNode
            | NodeKind::RegularExpressionNode
            | NodeKind::InterpolatedRegularExpressionNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::RangeNode
            | NodeKind::ConstantReadNode
            | NodeKind::ConstantPathNode
    )
}

// ---------------------------------------------------------------------------
// AbcSizeCalculator
// ---------------------------------------------------------------------------

/// A whitequark `send`/`csend` node that Prism folds into an abbreviated
/// assignment or a multiple-assignment target.
struct ImplicitCall<'pr> {
    safe_navigation: bool,
    /// The receiver of the hidden call, if it has one.
    receiver: Option<Node<'pr>>,
    /// The read (getter) name of the hidden call.
    name: &'pr [u8],
}

fn implicit_call<'pr>(node: &Node<'pr>) -> Option<ImplicitCall<'pr>> {
    macro_rules! from_call_write {
        ($accessor:ident) => {
            if let Some(call) = node.$accessor() {
                return Some(ImplicitCall {
                    safe_navigation: call.is_safe_navigation(),
                    receiver: call.receiver(),
                    name: call.read_name().as_slice(),
                });
            }
        };
    }
    macro_rules! from_index_write {
        ($accessor:ident) => {
            if let Some(call) = node.$accessor() {
                return Some(ImplicitCall {
                    safe_navigation: call.is_safe_navigation(),
                    receiver: call.receiver(),
                    name: b"[]",
                });
            }
        };
    }
    match node.kind() {
        NodeKind::CallOrWriteNode => from_call_write!(as_call_or_write_node),
        NodeKind::CallAndWriteNode => from_call_write!(as_call_and_write_node),
        NodeKind::CallOperatorWriteNode => from_call_write!(as_call_operator_write_node),
        NodeKind::IndexOrWriteNode => from_index_write!(as_index_or_write_node),
        NodeKind::IndexAndWriteNode => from_index_write!(as_index_and_write_node),
        NodeKind::IndexOperatorWriteNode => from_index_write!(as_index_operator_write_node),
        _ => {}
    }
    None
}

/// Port of `Metrics::Utils::AbcSizeCalculator.calculate`. Returns the
/// magnitude and the `<assignments, branches, conditions>` vector.
pub(crate) fn abc_size(
    node: &Node<'_>,
    discount_repeated_attributes: bool,
    it_parameter: bool,
) -> (f64, u32, u32, u32) {
    let mut calc = Abc {
        assignment: 0,
        branch: 0,
        condition: 0,
        it_parameter,
        csend: RepeatedCsend::default(),
        attrs: discount_repeated_attributes.then(AttrTrie::new),
    };
    calc.visit_depth_last(node, None, false);
    let (a, b, c) = (f64::from(calc.assignment), f64::from(calc.branch), f64::from(calc.condition));
    let magnitude = (a.mul_add(a, b.mul_add(b, c * c))).sqrt();
    ((magnitude * 100.0).round() / 100.0, calc.assignment, calc.branch, calc.condition)
}

struct Abc<'pr> {
    assignment: u32,
    branch: u32,
    condition: u32,
    /// Whether a bare `it` is the block's implicit parameter rather than a
    /// method call (`TargetRubyVersion` 3.4 and later).
    it_parameter: bool,
    csend: RepeatedCsend<'pr>,
    attrs: Option<AttrTrie<'pr>>,
}

impl<'pr> Abc<'pr> {
    fn visit_depth_last(&mut self, node: &Node<'pr>, parent: Option<&Node<'pr>>, in_pattern: bool) {
        for_each_child(node, |child| {
            self.visit_depth_last(child, Some(node), child_in_pattern(node, child, in_pattern));
        });
        self.calculate_node(node, parent, in_pattern);
    }

    fn calculate_node(&mut self, node: &Node<'pr>, parent: Option<&Node<'pr>>, in_pattern: bool) {
        // whitequark wraps an abbreviated assignment around a target node --
        // `(or-asgn (lvasgn :x) ...)`, `(or-asgn (send _ :foo) ...)` -- that
        // is visited and charged in its own right just before the wrapper.
        // Prism has no such node, so its contribution is added here.
        if is_shorthand_asgn(node.kind()) {
            if let Some(call) = implicit_call(node) {
                self.evaluate_branch(
                    call.receiver.as_ref(),
                    call.name,
                    call.safe_navigation,
                    node.span().start,
                    true,
                );
            } else if let Some(name) = lvasgn_name(node) {
                self.csend.reset(name);
                if is_capturing_variable(name) {
                    self.assignment += 1;
                }
            } else {
                self.assignment += 1;
            }
        }

        if self.attrs.is_some() {
            self.update_repeated_attribute(node);
        }

        if self.is_assignment(node, in_pattern) {
            self.assignment += 1;
        }

        if let Some(call) = node.as_call_node() {
            self.evaluate_branch(
                call.receiver().as_ref(),
                call.name().as_slice(),
                call.is_safe_navigation(),
                node.span().start,
                is_attribute_call(&call),
            );
        } else if let Some(target) = node.as_call_target_node() {
            self.evaluate_branch(
                Some(&target.receiver()),
                target.name().as_slice(),
                target.is_safe_navigation(),
                node.span().start,
                true,
            );
        } else if matches!(node.kind(), NodeKind::YieldNode | NodeKind::IndexTargetNode) {
            self.branch += 1;
        } else if node.kind() == NodeKind::LambdaNode {
            // whitequark builds `-> { ... }` as `(block (send nil :lambda)
            // (args) body)`, so the arrow itself is an argument-less call.
            self.evaluate_branch(None, b"lambda", false, node.span().start, true);
        } else if node.kind() == NodeKind::ItLocalVariableReadNode && !self.it_parameter {
            // Below Ruby 3.4 whitequark reads a bare `it` as `(send nil :it)`,
            // an argument-less call, so it is a branch like any other.
            self.evaluate_branch(None, b"it", false, node.span().start, true);
        } else if self.is_condition(node, parent) {
            self.evaluate_condition_node(node);
        }
    }

    /// `evaluate_branch_nodes`, plus `RepeatedAttributeDiscount`'s override.
    fn evaluate_branch(
        &mut self,
        receiver: Option<&Node<'pr>>,
        name: &'pr [u8],
        safe_navigation: bool,
        span_start: u32,
        attribute_call: bool,
    ) {
        if self.attrs.is_some()
            && attribute_call
            && self.discount_repeated_attribute(receiver, name)
        {
            return;
        }
        if is_comparison_method(name) {
            self.condition += 1;
        } else {
            self.branch += 1;
            if safe_navigation && !self.csend.discount(receiver, span_start) {
                self.condition += 1;
            }
        }
    }

    /// `evaluate_condition_node`.
    fn evaluate_condition_node(&mut self, node: &Node<'pr>) {
        let real_else = node
            .as_if_node()
            .and_then(|if_node| if_node.subsequent())
            .and_then(|subsequent| subsequent.as_else_node())
            .is_some_and(|else_node| is_real_else(&else_node))
            || node
                .as_unless_node()
                .and_then(|unless_node| unless_node.else_clause())
                .is_some_and(|else_node| is_real_else(&else_node));
        if real_else {
            self.condition += 1;
        }
        self.condition += 1;
    }

    /// `condition?`.
    fn is_condition(&self, node: &Node<'pr>, parent: Option<&Node<'pr>>) -> bool {
        if iterating_block(node, parent) == Some(false) || is_pattern_guard(node, parent) {
            return false;
        }
        is_counted_cyclomatic(node, self.it_parameter)
    }

    /// `assignment?`.
    fn is_assignment(&mut self, node: &Node<'pr>, in_pattern: bool) -> bool {
        let kind = node.kind();
        if matches!(kind, NodeKind::MultiWriteNode) || is_shorthand_asgn(kind) {
            self.compound_assignment(node);
            return false;
        }
        matches!(kind, NodeKind::ForNode)
            || is_setter_method(node)
            || self.is_simple_assignment(node, in_pattern)
            || is_argument(node)
    }

    /// `compound_assignment`: whitequark cannot see a setter method through a
    /// multiple or abbreviated assignment, so it charges every non-setter
    /// call-shaped child here instead.
    fn compound_assignment(&mut self, node: &Node<'pr>) {
        if let Some(multi) = node.as_multi_write_node() {
            let mut count = 0;
            each_multi_target(&multi.lefts(), multi.rest(), &multi.rights(), &mut |target| {
                count += u32::from(responds_to_setter_method(target) && !is_setter_method(target));
            });
            self.assignment += count;
            return;
        }
        // An abbreviated assignment's whitequark children are its target and
        // its value; the target of a `Call*Write`/`Index*Write` is a `send`.
        let mut count = u32::from(implicit_call(node).is_some());
        if let Some(value) = shorthand_value(node) {
            count += u32::from(responds_to_setter_method(&value) && !is_setter_method(&value));
        }
        self.assignment += count;
    }

    /// `simple_assignment?`.
    fn is_simple_assignment(&mut self, node: &Node<'pr>, in_pattern: bool) -> bool {
        if in_pattern {
            // whitequark's `match_var`/`match_rest`, which are not `lvasgn`.
            return false;
        }
        if let Some(name) = lvasgn_name(node) {
            // `LocalVariableTargetNode`/`LocalVariableWriteNode` only; the
            // abbreviated forms were handled by `compound_assignment`.
            self.csend.reset(name);
            return is_capturing_variable(name);
        }
        matches!(
            node.kind(),
            NodeKind::InstanceVariableWriteNode
                | NodeKind::InstanceVariableTargetNode
                | NodeKind::ClassVariableWriteNode
                | NodeKind::ClassVariableTargetNode
                | NodeKind::GlobalVariableWriteNode
                | NodeKind::GlobalVariableTargetNode
                | NodeKind::ConstantWriteNode
                | NodeKind::ConstantTargetNode
                | NodeKind::ConstantPathWriteNode
                | NodeKind::ConstantPathTargetNode
        )
    }
}

/// `RuboCop::AST::Node::COMPARISON_OPERATORS`.
fn is_comparison_method(name: &[u8]) -> bool {
    matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=" | b">" | b"<")
}

/// `capturing_variable?`.
fn is_capturing_variable(name: &[u8]) -> bool {
    !name.is_empty() && !name.starts_with(b"_")
}

/// `MethodDispatchNode#setter_method?` (`loc?(:operator)`): an assignment
/// call such as `foo.bar = 1` or `foo[1] = 2`.
fn is_setter_method(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|call| call.is_attribute_write())
}

/// `respond_to?(:setter_method?)`: whether whitequark would give the node a
/// class including `MethodDispatchNode` -- `send`/`csend`, `super`/`zsuper`,
/// `yield` and `defined?`.
///
/// A call that carries a literal block is wrapped in a `block` node
/// upstream, and `BlockNode` includes only `MethodIdentifierPredicates`, so
/// it does *not* respond. `&blk` is an ordinary `block_pass` argument of the
/// `send` itself and leaves the shape alone.
fn responds_to_setter_method(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::CallNode => {
            node.as_call_node().is_some_and(|call| !has_literal_block(call.block().as_ref()))
        }
        NodeKind::SuperNode => {
            node.as_super_node().is_some_and(|sup| !has_literal_block(sup.block().as_ref()))
        }
        NodeKind::ForwardingSuperNode => {
            node.as_forwarding_super_node().is_some_and(|sup| sup.block().is_none())
        }
        NodeKind::CallTargetNode
        | NodeKind::IndexTargetNode
        | NodeKind::YieldNode
        | NodeKind::DefinedNode => true,
        _ => false,
    }
}

fn has_literal_block(block: Option<&Node<'_>>) -> bool {
    block.is_some_and(|block| block.as_block_node().is_some())
}

/// `Node#argument_type?` combined with `capturing_variable?`.
fn is_argument(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::RequiredParameterNode => node
            .as_required_parameter_node()
            .is_some_and(|param| is_capturing_variable(param.name().as_slice())),
        NodeKind::OptionalParameterNode => node
            .as_optional_parameter_node()
            .is_some_and(|param| is_capturing_variable(param.name().as_slice())),
        NodeKind::RequiredKeywordParameterNode => node
            .as_required_keyword_parameter_node()
            .is_some_and(|param| is_capturing_variable(param.name().as_slice())),
        NodeKind::OptionalKeywordParameterNode => node
            .as_optional_keyword_parameter_node()
            .is_some_and(|param| is_capturing_variable(param.name().as_slice())),
        NodeKind::RestParameterNode => node.as_rest_parameter_node().is_some_and(|param| {
            param.name().is_some_and(|name| is_capturing_variable(name.as_slice()))
        }),
        NodeKind::KeywordRestParameterNode => {
            node.as_keyword_rest_parameter_node().is_some_and(|param| {
                param.name().is_some_and(|name| is_capturing_variable(name.as_slice()))
            })
        }
        NodeKind::BlockParameterNode => node.as_block_parameter_node().is_some_and(|param| {
            param.name().is_some_and(|name| is_capturing_variable(name.as_slice()))
        }),
        NodeKind::BlockLocalVariableNode => node
            .as_block_local_variable_node()
            .is_some_and(|param| is_capturing_variable(param.name().as_slice())),
        _ => false,
    }
}

/// `MlhsNode#assignments`: every target of a multiple assignment, with splats
/// unwrapped and nested targets flattened.
fn each_multi_target<'pr>(
    lefts: &ruby_ast::NodeList<'pr>,
    rest: Option<Node<'pr>>,
    rights: &ruby_ast::NodeList<'pr>,
    f: &mut impl FnMut(&Node<'pr>),
) {
    let mut visit = |target: &Node<'pr>| match target.kind() {
        NodeKind::SplatNode => {
            if let Some(splat) = target.as_splat_node() {
                match splat.expression() {
                    Some(expression) => f(&expression),
                    None => f(target),
                }
            }
        }
        NodeKind::MultiTargetNode => {
            if let Some(nested) = target.as_multi_target_node() {
                each_multi_target(&nested.lefts(), nested.rest(), &nested.rights(), f);
            }
        }
        _ => f(target),
    };
    for target in lefts {
        visit(&target);
    }
    if let Some(rest) = rest {
        visit(&rest);
    }
    for target in rights {
        visit(&target);
    }
}

/// The value of an abbreviated assignment, i.e. the second whitequark child
/// of an `op_asgn`/`or_asgn`/`and_asgn`.
fn shorthand_value<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    macro_rules! value_of {
        ($($accessor:ident),+ $(,)?) => {
            $(if let Some(inner) = node.$accessor() { return Some(inner.value()); })+
        };
    }
    value_of!(
        as_local_variable_or_write_node,
        as_local_variable_and_write_node,
        as_local_variable_operator_write_node,
        as_instance_variable_or_write_node,
        as_instance_variable_and_write_node,
        as_instance_variable_operator_write_node,
        as_class_variable_or_write_node,
        as_class_variable_and_write_node,
        as_class_variable_operator_write_node,
        as_global_variable_or_write_node,
        as_global_variable_and_write_node,
        as_global_variable_operator_write_node,
        as_constant_or_write_node,
        as_constant_and_write_node,
        as_constant_operator_write_node,
        as_constant_path_or_write_node,
        as_constant_path_and_write_node,
        as_constant_path_operator_write_node,
        as_call_or_write_node,
        as_call_and_write_node,
        as_call_operator_write_node,
        as_index_or_write_node,
        as_index_and_write_node,
        as_index_operator_write_node,
    );
    None
}

// ---------------------------------------------------------------------------
// RepeatedAttributeDiscount
// ---------------------------------------------------------------------------

/// One level of `@known_attributes`: either a chain root (the receiver the
/// chain starts from) or an attribute name.
#[derive(PartialEq, Eq)]
enum AttrKey<'pr> {
    /// `nil?` and `self` share one sub-table upstream.
    SelfOrImplicit,
    Local(&'pr [u8]),
    Instance(&'pr [u8]),
    Class(&'pr [u8]),
    Global(&'pr [u8]),
    Const(&'pr [u8]),
    Method(&'pr [u8]),
}

/// `@known_attributes`: a trie of attribute chains, flattened into an arena so
/// no nested maps have to be allocated per level.
struct AttrTrie<'pr> {
    levels: Vec<Vec<(AttrKey<'pr>, usize)>>,
}

impl<'pr> AttrTrie<'pr> {
    fn new() -> Self {
        Self { levels: vec![Vec::new()] }
    }

    fn child(&mut self, level: usize, key: AttrKey<'pr>, create: bool) -> Option<(usize, bool)> {
        if let Some(&(_, index)) = self.levels[level].iter().find(|(k, _)| *k == key) {
            return Some((index, false));
        }
        if !create {
            return None;
        }
        let index = self.levels.len();
        self.levels.push(Vec::new());
        self.levels[level].push((key, index));
        Some((index, true))
    }
}

/// `root_node?`: the receiver kinds a tracked attribute chain may start from.
fn attr_root_key<'pr>(node: Option<&Node<'pr>>) -> Option<AttrKey<'pr>> {
    let Some(node) = node else { return Some(AttrKey::SelfOrImplicit) };
    match node.kind() {
        NodeKind::SelfNode => Some(AttrKey::SelfOrImplicit),
        NodeKind::LocalVariableReadNode => {
            Some(AttrKey::Local(node.as_local_variable_read_node()?.name().as_slice()))
        }
        NodeKind::InstanceVariableReadNode => {
            Some(AttrKey::Instance(node.as_instance_variable_read_node()?.name().as_slice()))
        }
        NodeKind::ClassVariableReadNode => {
            Some(AttrKey::Class(node.as_class_variable_read_node()?.name().as_slice()))
        }
        NodeKind::GlobalVariableReadNode => {
            Some(AttrKey::Global(node.as_global_variable_read_node()?.name().as_slice()))
        }
        // Upstream keys a constant chain root by the constant node itself,
        // i.e. by its whole path. Prism only exposes the last segment of a
        // `ConstantPathNode`, so `A::Foo` and `B::Foo` share a chain root
        // here; the only effect is an extra discount in code that mixes two
        // same-named constants from different namespaces.
        NodeKind::ConstantReadNode => {
            Some(AttrKey::Const(node.as_constant_read_node()?.name().as_slice()))
        }
        NodeKind::ConstantPathNode => {
            Some(AttrKey::Const(node.as_constant_path_node()?.name()?.as_slice()))
        }
        _ => None,
    }
}

/// `attribute_call?`: `(call _receiver _method)` -- a call with no arguments.
fn is_attribute_call(call: &CallNode<'_>) -> bool {
    call.arguments().is_none()
}

impl<'pr> Abc<'pr> {
    /// `find_attributes`: walks the receiver chain, optionally creating the
    /// levels it does not find. Returns the level index and whether any level
    /// was created, or `None` when the chain is not a plain attribute chain
    /// (or a level was missing and `create` is false).
    fn find_attributes(
        &mut self,
        node: Option<&Node<'pr>>,
        create: bool,
        created: &mut bool,
    ) -> Option<usize> {
        if let Some(call) = node.and_then(Node::as_call_node) {
            if is_attribute_call(&call) {
                let receiver = call.receiver();
                let level = self.find_attributes(receiver.as_ref(), create, created)?;
                let key = AttrKey::Method(call.name().as_slice());
                let (index, made) = self.attrs.as_mut()?.child(level, key, create)?;
                *created |= made;
                return Some(index);
            }
        }
        let key = attr_root_key(node)?;
        let (index, made) = self.attrs.as_mut()?.child(0, key, create)?;
        *created |= made;
        Some(index)
    }

    /// `discount_repeated_attribute?`.
    fn discount_repeated_attribute(
        &mut self,
        receiver: Option<&Node<'pr>>,
        name: &'pr [u8],
    ) -> bool {
        let mut created = false;
        let Some(level) = self.find_attributes(receiver, true, &mut created) else {
            return false;
        };
        let Some(attrs) = self.attrs.as_mut() else { return false };
        let Some((_, made)) = attrs.child(level, AttrKey::Method(name), true) else {
            return false;
        };
        created |= made;
        !created
    }

    /// `update_repeated_attribute`: an assignment invalidates everything
    /// known about the variable or attribute it writes.
    fn update_repeated_attribute(&mut self, node: &Node<'pr>) {
        let Some(setter) = setter_to_getter(node) else { return };
        let mut created = false;
        match setter {
            Setter::Variable(key) => {
                let Some(attrs) = self.attrs.as_mut() else { return };
                let Some((level, _)) = attrs.child(0, key, false) else { return };
                attrs.levels[level].clear();
            }
            Setter::Attribute(receiver, name) => {
                let Some(level) = self.find_attributes(receiver.as_ref(), false, &mut created)
                else {
                    return;
                };
                let Some(attrs) = self.attrs.as_mut() else { return };
                attrs.levels[level].retain(|(key, _)| *key != AttrKey::Method(name));
            }
        }
    }
}

enum Setter<'pr> {
    Variable(AttrKey<'pr>),
    Attribute(Option<Node<'pr>>, &'pr [u8]),
}

/// `setter_to_getter`.
fn setter_to_getter<'pr>(node: &Node<'pr>) -> Option<Setter<'pr>> {
    match node.kind() {
        NodeKind::LocalVariableWriteNode => Some(Setter::Variable(AttrKey::Local(
            node.as_local_variable_write_node()?.name().as_slice(),
        ))),
        NodeKind::InstanceVariableWriteNode => Some(Setter::Variable(AttrKey::Instance(
            node.as_instance_variable_write_node()?.name().as_slice(),
        ))),
        NodeKind::ClassVariableWriteNode => Some(Setter::Variable(AttrKey::Class(
            node.as_class_variable_write_node()?.name().as_slice(),
        ))),
        NodeKind::GlobalVariableWriteNode => Some(Setter::Variable(AttrKey::Global(
            node.as_global_variable_write_node()?.name().as_slice(),
        ))),
        _ => {
            if is_shorthand_asgn(node.kind()) {
                // whitequark visits the wrapped target node (`(lvasgn :x)`,
                // `(send _ :foo)`) in its own right, so both shapes clear
                // what is known about the thing being written.
                return match implicit_call(node) {
                    Some(call) => Some(Setter::Attribute(call.receiver, call.name)),
                    None => shorthand_variable_key(node).map(Setter::Variable),
                };
            }
            let call = node.as_call_node()?;
            if !call.is_attribute_write() {
                return None;
            }
            let name = call.name().as_slice();
            let getter = name.strip_suffix(b"=")?;
            Some(Setter::Attribute(call.receiver(), getter))
        }
    }
}

/// The `VAR_SETTER_TO_GETTER` key of an abbreviated assignment to a plain
/// variable. Constants have no entry there, so they clear nothing.
fn shorthand_variable_key<'pr>(node: &Node<'pr>) -> Option<AttrKey<'pr>> {
    macro_rules! key {
        ($variant:ident, $($accessor:ident),+ $(,)?) => {
            $(if let Some(write) = node.$accessor() {
                return Some(AttrKey::$variant(write.name().as_slice()));
            })+
        };
    }
    key!(
        Local,
        as_local_variable_or_write_node,
        as_local_variable_and_write_node,
        as_local_variable_operator_write_node,
    );
    key!(
        Instance,
        as_instance_variable_or_write_node,
        as_instance_variable_and_write_node,
        as_instance_variable_operator_write_node,
    );
    key!(
        Class,
        as_class_variable_or_write_node,
        as_class_variable_and_write_node,
        as_class_variable_operator_write_node,
    );
    key!(
        Global,
        as_global_variable_or_write_node,
        as_global_variable_and_write_node,
        as_global_variable_operator_write_node,
    );
    None
}

// ---------------------------------------------------------------------------
// MethodComplexity mixin
// ---------------------------------------------------------------------------

/// The `AllowedMethods`/`AllowedPatterns` pair the three complexity cops
/// share. `IgnoredMethods`/`ExcludedMethods`/`IgnoredPatterns` are the
/// deprecated spellings; none of these cops declares one in `default.yml`,
/// so only the current names are read.
#[derive(Debug, Clone)]
pub(crate) struct AllowedNames {
    methods: Vec<String>,
    patterns: Vec<Regex>,
}

impl AllowedNames {
    pub(crate) fn new(options: &RuleOptions) -> Self {
        Self {
            methods: options.str_list("AllowedMethods"),
            patterns: options
                .str_list("AllowedPatterns")
                .iter()
                .filter_map(|pattern| Regex::new(pattern).ok())
                .collect(),
        }
    }

    /// `allowed_method?(name) || matches_allowed_pattern?(name)`.
    pub(crate) fn allows(&self, name: &[u8]) -> bool {
        if self.methods.is_empty() && self.patterns.is_empty() {
            return false;
        }
        let Ok(name) = std::str::from_utf8(name) else { return false };
        self.methods.iter().any(|allowed| allowed == name)
            || self.patterns.iter().any(|pattern| pattern.is_match(name))
    }
}

/// What `MethodComplexity#check_complexity` measures: the reported range
/// (`node.source_range`), the body to score, and the name for the message.
pub(crate) struct ComplexityTarget<'pr> {
    pub(crate) span: Span,
    pub(crate) body: Node<'pr>,
    pub(crate) name: &'pr [u8],
}

/// `on_def`/`on_defs`/`on_block`/`on_numblock`/`on_itblock`, fused: a method
/// definition, or a `define_method(:name) { ... }` block.
///
/// whitequark's `block` node wraps the call it belongs to, so its
/// `source_range` is the call's -- which in Prism is the `CallNode`'s own
/// span, the block included. `node.body` being nil (an empty method or
/// block) is upstream's early return, reported here as `None`.
pub(crate) fn complexity_target<'pr>(node: &Node<'pr>) -> Option<ComplexityTarget<'pr>> {
    if let Some(def) = node.as_def_node() {
        return Some(ComplexityTarget {
            span: node.span(),
            body: def.body()?,
            name: def.name().as_slice(),
        });
    }
    let call = node.as_call_node()?;
    if call.receiver().is_some()
        || call.is_safe_navigation()
        || call.name().as_slice() != b"define_method"
    {
        return None;
    }
    let block = call.block()?.as_block_node()?;
    let arguments = call.arguments()?.arguments();
    let mut arguments = arguments.iter();
    let argument = arguments.next()?;
    if arguments.next().is_some() {
        return None;
    }
    // The literal's source text rather than its unescaped value: `unescaped`
    // borrows from the node wrapper, and no `define_method` name in practice
    // carries an escape sequence.
    let name = match argument.kind() {
        NodeKind::SymbolNode => argument.as_symbol_node()?.value_loc()?.as_slice(),
        NodeKind::StringNode => argument.as_string_node()?.content_loc().as_slice(),
        _ => return None,
    };
    Some(ComplexityTarget { span: node.span(), body: block.body()?, name })
}

/// Ruby's `format('%.4g', value)`: four significant digits, fixed notation
/// while the decimal exponent stays in `-4..4`, trailing fractional zeros
/// dropped.
///
/// Ruby does not round the exact binary value the way C's `printf` does. Its
/// vendored `vsnprintf` asks David Gay's `dtoa` for the *shortest* decimal
/// that round-trips and rounds *that*, half to even: `Math.sqrt(21713)
/// .round(2)` is `147.34999999999999`, strictly below `147.35`, yet `%.4g`
/// prints `147.4`, and `104.45` prints `104.4`. Rust's `{:e}` produces the
/// same shortest decimal, so the rounding is done on its digits.
///
/// One more `dtoa` quirk shows up on real input: when the fifth significant
/// digit makes the shortest decimal an exact half, `dtoa` cannot settle the
/// direction in its floating-point fast path and falls through to the exact
/// big-integer one, which does not strip the trailing zeros it produced --
/// but only when the `double` sits *above* that decimal. `260.05` therefore
/// prints as `260.0` while `111.05`, whose `double` is just below, prints as
/// `111`.
pub(crate) fn format_g4(value: f64) -> String {
    if !value.is_finite() {
        return format!("{value}");
    }
    let shortest = format!("{value:e}");
    let (mantissa, exponent) = shortest.split_once('e').unwrap_or((shortest.as_str(), "0"));
    let mut exponent: i32 = exponent.parse().unwrap_or(0);
    let (sign, mantissa) = match mantissa.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", mantissa),
    };
    let mut digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).collect();
    let shortest_digits = digits.clone();
    let rounding = round_to_significant(&mut digits, 4);
    if rounding == Rounding::CarriedUp {
        exponent += 1;
    }
    let keep_zeros =
        rounding == Rounding::TiedDown && exceeds_decimal(value.abs(), &shortest_digits, exponent);
    if !keep_zeros {
        while digits.len() > 1 && digits.last() == Some(&b'0') {
            digits.pop();
        }
    }
    if digits == *b"0" {
        exponent = 0;
    }
    let body = if (-4..4).contains(&exponent) {
        fixed_notation(&digits, exponent)
    } else {
        let head = char::from(digits[0]);
        let tail = String::from_utf8_lossy(&digits[1..]).into_owned();
        let point = if tail.is_empty() { "" } else { "." };
        let exponent_sign = if exponent < 0 { '-' } else { '+' };
        format!("{head}{point}{tail}e{exponent_sign}{:02}", exponent.abs())
    };
    format!("{sign}{body}")
}

/// How `round_to_significant` resolved the digits it dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rounding {
    /// Nothing was dropped, or the dropped part was not an exact half and the
    /// result fit in the same number of digits.
    Plain,
    /// The dropped part was exactly half and the last kept digit was even, so
    /// the value was rounded down.
    TiedDown,
    /// Rounding up produced a new leading digit (`9999` -> `1000`, exponent
    /// one higher).
    CarriedUp,
}

/// Rounds `digits` (a bare digit string, one significant digit before an
/// implied point) to `keep` significant digits, half to even.
fn round_to_significant(digits: &mut Vec<u8>, keep: usize) -> Rounding {
    if digits.len() <= keep {
        return Rounding::Plain;
    }
    let first_dropped = digits[keep];
    let rest_nonzero = digits[keep + 1..].iter().any(|digit| *digit != b'0');
    digits.truncate(keep);
    let tied = first_dropped == b'5' && !rest_nonzero;
    let round_up = match first_dropped {
        b'0'..=b'4' => false,
        b'5' if !rest_nonzero => digits[keep - 1] % 2 == 1,
        _ => true,
    };
    if !round_up {
        return if tied { Rounding::TiedDown } else { Rounding::Plain };
    }
    for digit in digits.iter_mut().rev() {
        if *digit == b'9' {
            *digit = b'0';
        } else {
            *digit += 1;
            return Rounding::Plain;
        }
    }
    digits.insert(0, b'1');
    digits.truncate(keep);
    Rounding::CarriedUp
}

/// Whether `value` is strictly greater than the decimal `0.<digits> *
/// 10^(exponent + 1)`, compared exactly. Overflow of the exact comparison --
/// only reachable for magnitudes far outside a complexity metric -- answers
/// `false`, the branch that strips trailing zeros.
fn exceeds_decimal(value: f64, digits: &[u8], exponent: i32) -> bool {
    let bits = value.to_bits();
    let Ok(biased) = i32::try_from((bits >> 52) & 0x7ff) else { return false };
    let fraction = bits & ((1_u64 << 52) - 1);
    let (mantissa, binary_exponent) =
        if biased == 0 { (fraction, -1074) } else { (fraction | (1_u64 << 52), biased - 1075) };
    let mut decimal: u128 = 0;
    for digit in digits {
        let Some(next) = decimal.checked_mul(10) else { return false };
        decimal = next + u128::from(digit - b'0');
    }
    let Ok(length) = i32::try_from(digits.len()) else { return false };
    let decimal_exponent = exponent - (length - 1);
    let (mut left, mut right) = (u128::from(mantissa), decimal);
    let scaled = if binary_exponent < 0 {
        power(&mut right, 2, -binary_exponent)
    } else {
        power(&mut left, 2, binary_exponent)
    };
    let scaled = scaled
        && if decimal_exponent < 0 {
            power(&mut left, 10, -decimal_exponent)
        } else {
            power(&mut right, 10, decimal_exponent)
        };
    scaled && left > right
}

/// `target *= base.pow(exponent)`, reporting whether it fit.
fn power(target: &mut u128, base: u128, exponent: i32) -> bool {
    for _ in 0..exponent {
        match target.checked_mul(base) {
            Some(next) => *target = next,
            None => return false,
        }
    }
    true
}

/// `d[0].d[1..] * 10^exponent` written without an exponent, with no trailing
/// fractional zeros.
fn fixed_notation(digits: &[u8], exponent: i32) -> String {
    let text = String::from_utf8_lossy(digits);
    if exponent < 0 {
        let zeros = usize::try_from(-exponent - 1).unwrap_or(0);
        return format!("0.{}{text}", "0".repeat(zeros));
    }
    let integer_len = usize::try_from(exponent).unwrap_or(0) + 1;
    if digits.len() <= integer_len {
        let zeros = integer_len - digits.len();
        return format!("{text}{}", "0".repeat(zeros));
    }
    format!("{}.{}", &text[..integer_len], &text[integer_len..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruby_ast::Parsed;

    /// Every expected number in this module comes from RuboCop 1.82.1 itself,
    /// driven with `Max` below zero so the offense message carries the metric:
    ///
    /// ```ruby
    /// $LOAD_PATH.unshift 'rubocop-1.82.1/lib'
    /// require 'rubocop'
    /// def run(cop_name, code, extra = {})
    ///   klass = RuboCop::Cop::Registry.global.find_by_cop_name(cop_name)
    ///   cfg = RuboCop::Config.new(
    ///     { cop_name => { 'Enabled' => true, 'Max' => -1 }.merge(extra) }, '/fake/.rubocop.yml'
    ///   )
    ///   cop = klass.new(cfg)
    ///   src = RuboCop::ProcessedSource.new(code, 3.4, '(test)')
    ///   src.config = cfg
    ///   src.registry = RuboCop::Cop::Registry.global
    ///   RuboCop::Cop::Commissioner.new([cop], [], raise_error: true)
    ///           .investigate(src).offenses.map(&:message)
    /// end
    /// ```
    fn with_source<R>(code: &str, f: impl FnOnce(&SourceFile, &Node<'_>) -> R) -> R {
        let source = SourceFile::new("test.rb", code.as_bytes().to_vec());
        let parsed = Parsed::parse(&source);
        let root = parsed.root();
        assert!(!parsed.has_errors(), "snippet failed to parse: {code:?}");
        f(&source, &root)
    }

    fn find<'pr>(node: &Node<'pr>, kind: NodeKind) -> Option<Node<'pr>> {
        if node.kind() == kind {
            return Some(*node);
        }
        let mut found = None;
        for_each_child(node, |child| {
            if found.is_none() {
                found = find(child, kind);
            }
        });
        found
    }

    fn length_of(code: &str, kind: NodeKind, count_comments: bool, foldable: &[Foldable]) -> u32 {
        with_source(code, |src, root| {
            let node = find(root, kind).expect("snippet has the requested node");
            code_length_in(src, &node, count_comments, foldable)
        })
    }

    /// The body of the first `def`, which is what `MethodComplexity` and
    /// `AbcSize` are handed upstream (`check_complexity` calls
    /// `complexity(node.body)`).
    fn on_def_body<R>(code: &str, f: impl FnOnce(&Node<'_>) -> R) -> R {
        with_source(code, |_, root| {
            let def = find(root, NodeKind::DefNode).expect("snippet has a def");
            let body = def.as_def_node().and_then(|def| def.body()).expect("def has a body");
            f(&body)
        })
    }

    // -- CodeLengthCalculator ------------------------------------------------

    /// `run('Metrics/MethodLength', code, cfg)`.
    #[test]
    fn method_length_matches_rubocop() {
        let cases: &[(&str, bool, &[Foldable], u32)] = &[
            ("def foo\n  a\n\n  # c\n  b\nend\n", false, &[], 2),
            ("def foo\n  a\n\n  # c\n  b\nend\n", true, &[], 3),
            ("def foo\nend\n", false, &[], 0),
            ("def foo; a; end\n", false, &[], 1),
            ("def foo\n  def bar\n    a\n    b\n  end\n  c\nend\n", false, &[], 5),
            ("def foo\n  a\nrescue\n  b\nensure\n  c\nend\n", false, &[], 5),
            ("def foo\n  x = <<~TXT\n    one\n\n    two\n  TXT\n  x\nend\n", false, &[], 5),
            (
                "def foo\n  x = <<~TXT\n    one\n\n    two\n  TXT\n  x\nend\n",
                false,
                &[Foldable::Heredoc],
                2,
            ),
            ("def foo\n  bar([\n    1,\n    2,\n    3\n  ])\n  baz\nend\n", false, &[], 6),
            (
                "def foo\n  bar([\n    1,\n    2,\n    3\n  ])\n  baz\nend\n",
                false,
                &[Foldable::Array],
                2,
            ),
            (
                "def foo\n  bar(\n    a: 1,\n    b: 2\n  )\n  baz\nend\n",
                false,
                &[Foldable::Hash],
                2,
            ),
            (
                "def foo\n  bar({\n    a: 1,\n    b: 2\n  })\n  baz\nend\n",
                false,
                &[Foldable::Hash],
                2,
            ),
            (
                "def foo\n  bar(\n    1,\n    2\n  )\n  baz\nend\n",
                false,
                &[Foldable::MethodCall],
                2,
            ),
            ("def foo\n  bar(\"a\\nb\")\n  baz(<<~X)\n    hello\n  X\nend\n", false, &[], 4),
        ];
        for &(code, count_comments, foldable, expected) in cases {
            assert_eq!(
                length_of(code, NodeKind::DefNode, count_comments, foldable),
                expected,
                "{code:?} count_comments={count_comments}"
            );
        }
    }

    /// `run('Metrics/ClassLength', ...)` / `run('Metrics/ModuleLength', ...)`.
    ///
    /// `leading_comment` pins upstream's off-by-one: the comment on the first
    /// body line is *not* skipped, because `classlike_code_length` indexes the
    /// 0-based line array with 1-based line numbers.
    #[test]
    fn classlike_length_matches_rubocop() {
        let cases: &[(&str, NodeKind, bool, &[Foldable], u32)] = &[
            (
                "class Foo\n  def a\n    1\n  end\n\n  # c\n  def b\n    2\n  end\nend\n",
                NodeKind::ClassNode,
                false,
                &[],
                6,
            ),
            (
                "class Foo\n  def a\n    1\n  end\n\n  # c\n  def b\n    2\n  end\nend\n",
                NodeKind::ClassNode,
                true,
                &[],
                7,
            ),
            ("class Foo\n  # lead\n  def a\n    1\n  end\nend\n", NodeKind::ClassNode, false, &[], 4),
            (
                "class Foo\n  class Bar\n    def a\n      1\n    end\n  end\n\n  def b\n    2\n  end\nend\n",
                NodeKind::ClassNode,
                false,
                &[],
                4,
            ),
            (
                "class Foo\n  class Bar\n    def a\n      1\n    end\n  end\nend\n",
                NodeKind::ClassNode,
                false,
                &[],
                0,
            ),
            ("class Foo\nend\n", NodeKind::ClassNode, false, &[], 0),
            (
                "class Foo\n  A = <<~TXT\n    one\n    two\n    three\n  TXT\n  def b\n    2\n  end\nend\n",
                NodeKind::ClassNode,
                false,
                &[Foldable::Heredoc],
                4,
            ),
            (
                "module Foo\n  def a\n    1\n  end\n\n  def b\n    2\n  end\nend\n",
                NodeKind::ModuleNode,
                false,
                &[],
                6,
            ),
            (
                "module Foo\n  module Bar\n    def a\n      1\n    end\n  end\nend\n",
                NodeKind::ModuleNode,
                false,
                &[],
                0,
            ),
            (
                "module Foo\n  module Bar\n    def a\n      1\n    end\n  end\n\n  def b\n    2\n  end\nend\n",
                NodeKind::ModuleNode,
                false,
                &[],
                4,
            ),
        ];
        for &(code, kind, count_comments, foldable, expected) in cases {
            assert_eq!(
                length_of(code, kind, count_comments, foldable),
                expected,
                "{code:?} count_comments={count_comments}"
            );
        }
    }

    /// `run('Metrics/BlockLength', ...)`.
    #[test]
    fn block_length_matches_rubocop() {
        let cases: &[(&str, &[Foldable], u32)] = &[
            ("foo do\n  a\n\n  # c\n  b\nend\n", &[], 2),
            ("foo do\n  bar([\n    1,\n    2\n  ])\n  baz\nend\n", &[Foldable::Array], 2),
            ("foo do\n  bar([\n    1,\n    2\n  ])\n  baz\nend\n", &[], 5),
            ("foo do\n  x = <<~TXT\n    one\n    two\n  TXT\n  x\nend\n", &[], 5),
        ];
        for &(code, foldable, expected) in cases {
            assert_eq!(length_of(code, NodeKind::BlockNode, false, foldable), expected, "{code:?}");
        }
    }

    #[test]
    fn foldable_from_config_drops_unknown_entries() {
        let values = ["array".to_string(), "nope".to_string(), "method_call".to_string()];
        assert_eq!(Foldable::from_config(&values), vec![Foldable::Array, Foldable::MethodCall]);
        assert_eq!(Foldable::from_config(&[]), Vec::<Foldable>::new());
    }

    // -- MethodComplexity ----------------------------------------------------

    /// `run('Metrics/CyclomaticComplexity', code)`.
    #[test]
    fn cyclomatic_matches_rubocop() {
        let cases: &[(&str, u32)] = &[
            ("def foo\n  a\n  b\nend\n", 1),
            ("def foo\n  if a\n    b\n  else\n    c\n  end\nend\n", 2),
            ("def foo\n  if a\n    b\n  elsif c\n    d\n  end\nend\n", 3),
            ("def foo\n  a ? b : c\nend\n", 2),
            ("def foo\n  b unless a\nend\n", 2),
            ("def foo\n  while a\n    b\n  end\n  c until d\nend\n", 3),
            ("def foo\n  for i in 1..3\n    p i\n  end\nend\n", 2),
            ("def foo\n  case x\n  when 1 then a\n  when 2 then b\n  else c\n  end\nend\n", 3),
            ("def foo\n  case x\n  in [1, 2] then a\n  in Integer then b\n  end\nend\n", 3),
            ("def foo\n  a && b || c\nend\n", 3),
            ("def foo\n  a.each { |x| p x }\n  b.tap { |x| p x }\nend\n", 2),
            ("def foo\n  a.map(&:to_s)\n  b.tap(&:to_s)\nend\n", 2),
            ("def foo\n  a\nrescue\n  b\nend\n", 2),
            ("def foo\n  a rescue b\nend\n", 2),
            ("def foo\n  begin\n    a\n  rescue A\n    b\n  rescue B\n    c\n  end\nend\n", 2),
            ("def foo(v)\n  v&.a\n  v&.b\n  v = 1\n  v&.c\nend\n", 3),
            ("def foo\n  x ||= 1\n  @y &&= 2\n  z += 3\nend\n", 3),
            ("def foo(v)\n  v&.bar ||= 1\nend\n", 3),
            ("def foo\n  [1, 2].map { |x| x.odd? ? x : -x }.select { |x| x > 0 }\nend\n", 4),
            (
                "def each_child_node(*types)\n  unless block_given?\n    return to_enum(__method__, *types)\n  end\n\n  children.each do |child|\n    next unless child.is_a?(Node)\n\n    yield child if types.empty? ||\n                   types.include?(child.type)\n  end\n\n  self\nend\n",
                6,
            ),
        ];
        for &(code, expected) in cases {
            assert_eq!(on_def_body(code, |body| cyclomatic(body, true)), expected, "{code:?}");
        }
    }

    /// `run('Metrics/PerceivedComplexity', code)`.
    #[test]
    fn perceived_matches_rubocop() {
        let cases: &[(&str, u32)] = &[
            ("def foo\n  a\n  b\nend\n", 1),
            ("def foo\n  if a\n    b\n  else\n    c\n  end\nend\n", 3),
            ("def foo\n  if a\n    b\n  elsif c\n    d\n  end\nend\n", 4),
            ("def foo\n  if a\n    b\n  elsif c\n    d\n  else\n    e\n  end\nend\n", 4),
            ("def foo\n  a ? b : c\nend\n", 2),
            ("def foo\n  unless a\n    b\n  else\n    c\n  end\nend\n", 3),
            (
                "def foo\n  case var\n  when 1 then func_one\n  when 2 then func_two\n  when 3 then func_three\n  when 4..10 then func_other\n  end\nend\n",
                3,
            ),
            ("def foo\n  case\n  when a then b\n  when c then d\n  else e\n  end\nend\n", 4),
            ("def foo\n  case var\n  when 1 then a\n  else b\n  end\nend\n", 2),
            // 1.88 gave `case`/`in` branches the `when` discount: a
            // structural pattern still costs 1, a constant only 0.2, so
            // `(1 + 0.2).round` is one point, not two.
            ("def foo\n  case x\n  in [1, 2] then a\n  in Integer then b\n  end\nend\n", 2),
            ("def foo\n  a.each { |x| p x }\n  b.tap { |x| p x }\nend\n", 2),
            (
                "def my_method\n  if cond\n    case var\n    when 1 then func_one\n    when 2 then func_two\n    when 3 then func_three\n    when 4..10 then func_other\n    end\n  else\n    do_something until a && b\n  end\nend\n",
                7,
            ),
        ];
        for &(code, expected) in cases {
            assert_eq!(on_def_body(code, |body| perceived(body, true)), expected, "{code:?}");
        }
    }

    #[test]
    fn format_g4_matches_ruby() {
        // `format('%.4g', value)` under Ruby 3.4.
        let cases: &[(f64, &str)] = &[
            (147.35, "147.4"),
            (188.35, "188.4"),
            (104.45, "104.4"),
            (102.65, "102.6"),
            (106.55, "106.6"),
            (101.45, "101.4"),
            (4.24, "4.24"),
            (1.0, "1"),
            (0.0, "0"),
            (17.0, "17"),
            (1.3, "1.3"),
            (10.3, "10.3"),
            (42.43, "42.43"),
            (424.3, "424.3"),
            (4243.0, "4243"),
            (42430.0, "4.243e+04"),
            (0.000_123_4, "0.0001234"),
            (108.0, "108"),
            (107.8, "107.8"),
            // `dtoa`'s exact-half fall-through: zeros survive only when the
            // `double` sits above the shortest decimal.
            (260.05, "260.0"),
            (111.05, "111"),
            (2.0005, "2.000"),
            (1.0005, "1"),
            (12.005, "12.00"),
            (0.10005, "0.1000"),
            (260.15, "260.2"),
            (1.2005, "1.2"),
            (105.05, "105"),
        ];
        for &(value, expected) in cases {
            assert_eq!(format_g4(value), expected, "{value}");
        }
    }

    // -- AbcSizeCalculator ---------------------------------------------------

    /// `run('Metrics/AbcSize', code, 'CountRepeatedAttributes' => true)`; the
    /// message reads `[<a, b, c> magnitude/max]`.
    #[test]
    fn abc_size_matches_rubocop() {
        let cases: &[(&str, u32, u32, u32, f64)] = &[
            ("def foo(a)\n  b = a.bar\n  b + 1\nend\n", 1, 2, 0, 2.24),
            ("def foo\n  nil\nend\n", 0, 0, 0, 0.0),
            ("def foo(a, b = 1, *c, d:, e: 2, **f, &g)\n  nil\nend\n", 0, 0, 0, 0.0),
            ("def foo(_a, b)\n  nil\nend\n", 0, 0, 0, 0.0),
            ("def foo(o)\n  o.bar = 1\n  o[0] = 2\n  @x = 3\nend\n", 3, 2, 0, 3.61),
            ("def foo\n  a, b = 1, 2\n  c, d.e = 3, 4\nend\n", 4, 2, 0, 4.47),
            ("def foo\n  x ||= 1\n  y ||= bar\n  @z ||= baz.qux\nend\n", 5, 3, 3, 6.56),
            ("def foo(o)\n  o.bar += 1\nend\n", 1, 1, 0, 1.41),
            ("def foo(a, b)\n  a == b\n  a < b\n  a <=> b\nend\n", 0, 1, 2, 2.24),
            ("def foo(v)\n  v&.a\n  v&.b\n  v = 1\n  v&.c\nend\n", 1, 3, 2, 3.74),
            ("def foo(a)\n  if a\n    b\n  else\n    c\n  end\nend\n", 0, 2, 2, 2.83),
            ("def foo(x)\n  case x\n  when 1 then a\n  when 2 then b\n  else c\n  end\nend\n", 0, 3, 2, 3.61),
            ("def foo\n  for i in 1..3\n    p i\n  end\nend\n", 2, 1, 1, 2.45),
            ("def foo\n  yield 1\n  [1].each { |x| p x }\n  bar(&:baz)\nend\n", 1, 4, 1, 4.24),
            ("def foo\n  a\nrescue => e\n  b(e)\nend\n", 1, 2, 1, 2.45),
            (
                "def search\n  @posts = model.active.visible_by(current_user)\n            .search(params[:q])\n  @posts = model.some_process(@posts, current_user)\n  @posts = model.another_process(@posts, current_user)\n\n  render 'pages/search/page'\nend\n",
                3,
                14,
                0,
                14.32,
            ),
        ];
        for &(code, assignment, branch, condition, magnitude) in cases {
            let got = on_def_body(code, |body| abc_size(body, false, true));
            assert_eq!(
                (got.1, got.2, got.3),
                (assignment, branch, condition),
                "vector for {code:?}"
            );
            assert!((got.0 - magnitude).abs() < 1e-9, "magnitude for {code:?}: {got:?}");
        }
    }

    /// `run('Metrics/AbcSize', code, 'CountRepeatedAttributes' => false)`.
    #[test]
    fn abc_size_discounts_repeated_attributes() {
        let cases: &[(&str, u32, u32, u32)] = &[
            (
                "def search\n  @posts = model.active.visible_by(current_user)\n            .search(params[:q])\n  @posts = model.some_process(@posts, current_user)\n  @posts = model.another_process(@posts, current_user)\n\n  render 'pages/search/page'\nend\n",
                3,
                10,
                0,
            ),
            ("def foo\n  a.b\n  a.b\n  a.b.c\nend\n", 0, 3, 0),
            (
                "def foo\n  xx.foo.bar\n  xx.foo.baz\n  self.xx = any\n  xx.foo.baz\n  self.xx.foo.baz\nend\n",
                1,
                9,
                0,
            ),
            (
                "def foo\n  obj = build\n  obj.baz.qux\n  obj.baz.qux\n  obj = other\n  obj.baz.qux\nend\n",
                2,
                6,
                0,
            ),
            ("def foo\n  a.b(1)\n  a.b(1)\nend\n", 0, 3, 0),
        ];
        for &(code, assignment, branch, condition) in cases {
            let got = on_def_body(code, |body| abc_size(body, true, true));
            assert_eq!(
                (got.1, got.2, got.3),
                (assignment, branch, condition),
                "vector for {code:?}"
            );
        }
    }
}
