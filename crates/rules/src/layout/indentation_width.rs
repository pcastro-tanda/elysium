//! `Layout/IndentationWidth`, ported from RuboCop's `lib/rubocop/cop/layout/indentation_width.rb`
//! plus its `Alignment`, `EndKeywordAlignment`, `CheckAssignment`, and `AllowedPattern` mixins.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{walk, LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;
use std::collections::HashSet;

/// RuboCop's `MSG`.
const MSG: &str =
    "Use %<configured_indentation_width>d (not %<indentation>d) spaces for%<name>s indentation.";

/// `Layout/DefEndAlignment`'s `EnforcedStyleAlignWith` default.
const DEF_END_ALIGNMENT_DEFAULT: &str = "start_of_line";
/// `Layout/EndAlignment`'s `EnforcedStyleAlignWith` default.
const END_ALIGNMENT_DEFAULT: &str = "keyword";
/// `Layout/AccessModifierIndentation`'s `EnforcedStyle` default.
const ACCESS_MODIFIER_STYLE_DEFAULT: &str = "indent";
/// `Layout/IndentationConsistency`'s `EnforcedStyle` default.
const CONSISTENCY_STYLE_DEFAULT: &str = "normal";

/// Checks for indentation that doesn't use the configured number of spaces.
#[derive(Debug, Clone)]
pub struct IndentationWidth {
    width: i64,
    allowed_patterns: Vec<regex::Regex>,
    /// `Layout/IndentationConsistency` `EnforcedStyle == 'indented_internal_methods'`.
    indented_internal_methods: bool,
    /// `Layout/AccessModifierIndentation` `EnforcedStyle == 'outdent'`.
    access_modifier_outdent: bool,
    /// `Layout/EndAlignment` `EnforcedStyleAlignWith`.
    end_alignment: EndAlignment,
    /// `Layout/DefEndAlignment` `EnforcedStyleAlignWith == 'def'`.
    def_end_alignment_is_def: bool,
    /// Byte ranges already flagged this file, autocorrect target narrowed to the first one
    /// (RuboCop's `other_offense_in_same_range?`).
    offense_ranges: Vec<Span>,
    /// Start offsets of nodes already handled by `check_assignment`/`adjacent_def_modifier?`
    /// and that must be skipped when the generic traversal reaches them directly
    /// (RuboCop's `ignore_node`/`ignored_node?`).
    ignored: HashSet<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EndAlignment {
    Keyword,
    Variable,
    StartOfLine,
}

impl IndentationWidth {
    /// RuboCop's `allowed_line?`: the text of `span`'s line matches one of `AllowedPatterns`.
    fn allowed_line(&self, ctx: &Context<'_>, span: Span) -> bool {
        if self.allowed_patterns.is_empty() {
            return false;
        }
        let line = ctx.line_text(ctx.line_col(span.start).line);
        let line = String::from_utf8_lossy(line);
        self.allowed_patterns.iter().any(|re| re.is_match(&line))
    }
}

/// RuboCop's `Util#begins_its_line?`: `span` is the first non-blank token on its line.
fn begins_its_line(ctx: &Context<'_>, span: Span) -> bool {
    let lc = ctx.line_col(span.start);
    let line = ctx.line_text(lc.line);
    match line.iter().position(|&b| !b.is_ascii_whitespace()) {
        Some(pos) => u32::try_from(pos).unwrap_or(u32::MAX) == lc.column,
        None => false,
    }
}

fn same_line(ctx: &Context<'_>, a: Span, b: Span) -> bool {
    ctx.line_col(a.start).line == ctx.line_col(b.start).line
}

/// RuboCop's `effective_column`: the raw column, except on line 1 of a file starting with
/// a UTF-8 byte-order mark, where the BOM character itself is not counted.
fn column_of(ctx: &Context<'_>, span: Span) -> i64 {
    let lc = ctx.line_col(span.start);
    let mut column = i64::from(lc.column);
    if lc.line == 1 && ctx.source().bytes().starts_with(&[0xEF, 0xBB, 0xBF]) {
        column -= 1;
    }
    column
}

impl IndentationWidth {
    /// RuboCop's `check_indentation`. `base` is the location the body is expected to be
    /// indented from; `style` names the message suffix (`"normal"` prints nothing).
    fn check_indentation(
        &mut self,
        ctx: &mut Context<'_>,
        base: Span,
        body: Option<Node<'_>>,
        style: &'static str,
    ) {
        let Some(body) = body else { return };
        // Prism's *implicit* rescue/ensure wrapper (no literal `begin` keyword: a `def`/
        // `block`/etc. body that goes straight into `rescue`/`ensure`) spans the whole
        // enclosing construct rather than just its leading statements. RuboCop's
        // `:rescue`/`:ensure`-typed whitequark body node, by contrast, is exactly its
        // leading body, so retarget to `statements()` (RuboCop's `check_rescue?`/
        // `ensure`-gate: skip entirely when there is no leading body to check).
        let body = if let Node::BeginNode { .. } = body {
            let begin = body.as_begin_node().expect("kind matched");
            if begin.begin_keyword_loc().is_none() {
                match begin.statements() {
                    Some(s) => s.as_node(),
                    None => return,
                }
            } else {
                body
            }
        } else {
            body
        };
        if self.ignored.contains(&body.span().start) {
            return;
        }
        if !self.indentation_to_check(ctx, base, &body) {
            return;
        }
        let body_span = body.span();
        let indentation = column_of(ctx, body_span) - column_of(ctx, base);
        let delta = self.width - indentation;
        if delta == 0 {
            return;
        }
        self.offense(ctx, body, indentation, style);
    }

    /// RuboCop's `skip_check?` (the rescue/ensure `check_rescue?` gate is handled by
    /// [`Self::check_indentation`]'s upfront retargeting).
    fn indentation_to_check(&self, ctx: &Context<'_>, base: Span, body: &Node<'_>) -> bool {
        if self.allowed_line(ctx, base) {
            return false;
        }
        let body_span = body.span();
        if same_line(ctx, body_span, base) {
            return false;
        }
        if starts_with_access_modifier(body) {
            return false;
        }
        if !begins_its_line(ctx, body_span) {
            return false;
        }
        true
    }

    /// RuboCop's `offense`: reassigns to the first statement for multi-statement bodies,
    /// computes the message and offense range, and attaches an autocorrect fix unless another
    /// offense already covers the same range.
    fn offense(
        &mut self,
        ctx: &mut Context<'_>,
        body: Node<'_>,
        indentation: i64,
        style: &'static str,
    ) {
        let body_span = body.span();
        let fix_target = narrow_for_fix(body);
        let fix_span = fix_target.span();

        let name = if style == "normal" { String::new() } else { format!(" {style}") };
        let message = MSG
            .replace("%<configured_indentation_width>d", &self.width.to_string())
            .replace("%<indentation>d", &indentation.to_string())
            .replace("%<name>s", &name);

        let range = offending_range(body_span.start, indentation);

        let overlaps = self.offense_ranges.iter().any(|r| r.contains(fix_span));
        if !overlaps {
            self.offense_ranges.push(fix_span);
        }

        let column_delta = self.width - indentation;
        if !overlaps {
            if let Some(edits) = build_alignment_edits(ctx, &fix_target, column_delta) {
                ctx.report_with_fix(
                    &Self::META,
                    range,
                    message,
                    Fix { applicability: Applicability::Safe, edits },
                );
                return;
            }
        }
        ctx.report(&Self::META, range, message);
    }
    /// RuboCop's `on_def`/`on_defs`.
    fn on_def(&mut self, ctx: &mut Context<'_>, node: &Node<'_>) {
        if self.ignored.contains(&node.span().start) {
            return;
        }
        let def = node.as_def_node().expect("kind matched");
        self.check_indentation(ctx, def.def_keyword_loc().span(), def.body(), "normal");
    }

    fn on_while_node(&mut self, ctx: &mut Context<'_>, node: &Node<'_>) {
        let w = node.as_while_node().expect("kind matched");
        self.on_while_until(
            ctx,
            w.keyword_loc().span(),
            w.predicate().span(),
            w.statements().map(|s| s.as_node()),
            node.span(),
        );
    }

    fn on_until_node(&mut self, ctx: &mut Context<'_>, node: &Node<'_>) {
        let u = node.as_until_node().expect("kind matched");
        self.on_while_until(
            ctx,
            u.keyword_loc().span(),
            u.predicate().span(),
            u.statements().map(|s| s.as_node()),
            node.span(),
        );
    }

    /// RuboCop's `on_block`'s `check_members` call for lambdas (Prism has no numblock/itblock
    /// distinction; both share `LambdaNode`/`BlockNode`).
    fn on_lambda(&mut self, ctx: &mut Context<'_>, node: &Node<'_>) {
        let lambda = node.as_lambda_node().expect("kind matched");
        let end_span = lambda.closing_loc().span();
        if !begins_its_line(ctx, end_span) {
            return;
        }
        self.check_indentation(ctx, end_span, lambda.body(), "normal");
        if self.indented_internal_methods {
            self.check_members(ctx, end_span, lambda.body());
        }
    }

    /// RuboCop's `on_resbody`.
    fn on_rescue(&mut self, ctx: &mut Context<'_>, node: &Node<'_>) {
        let rescue = node.as_rescue_node().expect("kind matched");
        self.check_indentation(
            ctx,
            rescue.keyword_loc().span(),
            rescue.statements().map(|s| s.as_node()),
            "normal",
        );
    }

    /// RuboCop's `on_ensure`.
    fn on_ensure(&mut self, ctx: &mut Context<'_>, node: &Node<'_>) {
        let ensure = node.as_ensure_node().expect("kind matched");
        self.check_indentation(
            ctx,
            ensure.ensure_keyword_loc().span(),
            ensure.statements().map(|s| s.as_node()),
            "normal",
        );
    }

    /// RuboCop's `on_for` (aliased to `on_resbody` upstream; Prism gives `ForNode` its own
    /// keyword/statements accessors instead).
    fn on_for(&mut self, ctx: &mut Context<'_>, node: &Node<'_>) {
        let for_node = node.as_for_node().expect("kind matched");
        self.check_indentation(
            ctx,
            for_node.for_keyword_loc().span(),
            for_node.statements().map(|s| s.as_node()),
            "normal",
        );
    }

    /// RuboCop's `on_class`/`on_sclass`/`on_module` (Class/Module/SingletonClass share the same
    /// shape: skip when the whole thing is on one line, else run `check_members` from the
    /// class-like keyword).
    fn on_class_like(
        &mut self,
        ctx: &mut Context<'_>,
        whole: Span,
        keyword: Span,
        body: Option<Node<'_>>,
    ) {
        if let Some(b) = &body {
            if same_line(ctx, whole, b.span()) {
                return;
            }
        }
        self.check_members(ctx, keyword, body);
    }

    /// RuboCop's `on_case`.
    fn on_case(&mut self, ctx: &mut Context<'_>, node: &Node<'_>) {
        let case_node = node.as_case_node().expect("kind matched");
        let mut last_when_kw: Option<Span> = None;
        for branch in &case_node.conditions() {
            if let Node::WhenNode { .. } = branch {
                let when = branch.as_when_node().expect("kind matched");
                let kw = when.keyword_loc().span();
                self.check_indentation(ctx, kw, when.statements().map(|s| s.as_node()), "normal");
                last_when_kw = Some(kw);
            }
        }
        if let (Some(base), Some(else_node)) = (last_when_kw, case_node.else_clause()) {
            self.check_indentation(
                ctx,
                base,
                else_node.statements().map(|s| s.as_node()),
                "normal",
            );
        }
    }

    /// RuboCop's `on_case_match`.
    fn on_case_match(&mut self, ctx: &mut Context<'_>, node: &Node<'_>) {
        let case_match = node.as_case_match_node().expect("kind matched");
        let mut last_in_kw: Option<Span> = None;
        for branch in &case_match.conditions() {
            if let Node::InNode { .. } = branch {
                let in_node = branch.as_in_node().expect("kind matched");
                let kw = in_node.in_loc().span();
                self.check_indentation(
                    ctx,
                    kw,
                    in_node.statements().map(|s| s.as_node()),
                    "normal",
                );
                last_in_kw = Some(kw);
            }
        }
        if let (Some(base), Some(else_node)) = (last_in_kw, case_match.else_clause()) {
            self.check_indentation(
                ctx,
                base,
                else_node.statements().map(|s| s.as_node()),
                "normal",
            );
        }
    }

    /// RuboCop's `on_kwbegin`/`on_rescue` (the `:rescue`/`:kwbegin` wrapper node, unified in
    /// Prism as `BeginNode`).
    fn on_begin(&mut self, ctx: &mut Context<'_>, node: &Node<'_>) {
        let begin = node.as_begin_node().expect("kind matched");
        if let (Some(_begin_kw), Some(end_kw)) =
            (begin.begin_keyword_loc(), begin.end_keyword_loc())
        {
            let end_span = end_kw.span();
            if begins_its_line(ctx, end_span) {
                // RuboCop's `check_rescue?`: only the leading body (before any `rescue`) is
                // checked here; an empty leading body is skipped entirely (the `rescue`/`else`/
                // `ensure` clauses are each checked independently by their own node handler).
                self.check_indentation(
                    ctx,
                    end_span,
                    begin.statements().map(|s| s.as_node()),
                    "normal",
                );
            }
        }
        if let Some(else_clause) = begin.else_clause() {
            self.check_indentation(
                ctx,
                else_clause.else_keyword_loc().span(),
                else_clause.statements().map(|s| s.as_node()),
                "normal",
            );
        }
    }

    fn on_if(&mut self, ctx: &mut Context<'_>, node: &Node<'_>, base: Span) {
        let if_node = node.as_if_node().expect("kind matched");
        if if_node.end_keyword_loc().is_none() {
            // Ternary or modifier form; neither has a body to check.
            return;
        }
        self.check_indentation(ctx, base, if_node.statements().map(|s| s.as_node()), "normal");
        if let Some(subsequent) = if_node.subsequent() {
            if let Node::ElseNode { .. } = &subsequent {
                let else_node = subsequent.as_else_node().expect("kind matched");
                self.check_indentation(
                    ctx,
                    else_node.else_keyword_loc().span(),
                    else_node.statements().map(|s| s.as_node()),
                    "normal",
                );
            }
            // An `elsif` gets its own `on_if` call when the traversal reaches it.
        }
    }

    fn on_unless(&mut self, ctx: &mut Context<'_>, node: &Node<'_>) {
        let unless_node = node.as_unless_node().expect("kind matched");
        if unless_node.end_keyword_loc().is_none() {
            return;
        }
        let base = unless_node.location().span();
        self.check_indentation(ctx, base, unless_node.statements().map(|s| s.as_node()), "normal");
        if let Some(else_node) = unless_node.else_clause() {
            self.check_indentation(
                ctx,
                else_node.else_keyword_loc().span(),
                else_node.statements().map(|s| s.as_node()),
                "normal",
            );
        }
    }

    fn on_while_until(
        &mut self,
        ctx: &mut Context<'_>,
        keyword: Span,
        predicate: Span,
        statements: Option<Node<'_>>,
        base: Span,
    ) {
        if self.ignored.contains(&keyword.start) {
            return;
        }
        // RuboCop's `single_line_condition?`: skip when the predicate itself starts on a
        // different line than the keyword (rare multi-line-condition edge case).
        if ctx.line_col(keyword.start).line != ctx.line_col(predicate.start).line {
            return;
        }
        self.check_indentation(ctx, base, statements, "normal");
    }

    /// RuboCop's `check_members`/`select_check_member`/`check_members_for_normal_style`/
    /// `check_members_for_indented_internal_methods_style`, for class/module/sclass/block/
    /// lambda bodies.
    fn check_members(&mut self, ctx: &mut Context<'_>, base: Span, member: Option<Node<'_>>) {
        // Extracted independently of `member`'s ownership (a `NodeList` does not borrow from
        // the `Node` it was read off of), so `member` stays free to be moved below.
        let body_list = member.as_ref().and_then(|m| {
            if let Node::StatementsNode { .. } = m {
                Some(m.as_statements_node().expect("kind matched").body())
            } else {
                None
            }
        });

        let first_is_modifier =
            body_list.as_ref().and_then(ruby_ast::NodeList::first).filter(is_bare_access_modifier);
        let select = if let Some(first) = first_is_modifier {
            if self.access_modifier_outdent {
                None
            } else {
                Some(first)
            }
        } else {
            member
        };
        self.check_indentation(ctx, base, select, "normal");

        let Some(list) = body_list else { return };
        if self.indented_internal_methods {
            let mut previous_modifier: Option<Span> = None;
            for child in &list {
                if is_special_modifier(&child) {
                    previous_modifier = Some(child.span());
                } else if let Some(prev) = previous_modifier.take() {
                    self.check_indentation(ctx, prev, Some(child), "indented_internal_methods");
                }
            }
        } else {
            for child in &list {
                if is_bare_access_modifier(&child) {
                    continue;
                }
                self.check_indentation(ctx, base, Some(child), "normal");
            }
        }
    }

    /// RuboCop's `check_assignment`: when the value assigned is (a chain rooted in) an
    /// `if`/`while`/`until` node, check it against a base chosen by `Layout/EndAlignment`'s
    /// style instead of the ordinary auto-visited base.
    fn check_assignment(&mut self, ctx: &mut Context<'_>, whole: Span, value: Node<'_>) {
        let rhs = match value {
            Node::IfNode { .. } | Node::WhileNode { .. } | Node::UntilNode { .. } => value,
            _ => return,
        };
        let rhs_span = rhs.span();
        let variable_alignment = match self.end_alignment {
            EndAlignment::Keyword => false,
            _ => ctx.line_col(rhs_span.start).line <= ctx.line_col(whole.start).line,
        };
        let base = if variable_alignment { whole } else { rhs_span };
        self.ignored.insert(rhs_span.start);
        match rhs {
            Node::IfNode { .. } => self.on_if(ctx, &rhs, base),
            Node::WhileNode { .. } => {
                let w = rhs.as_while_node().expect("kind matched");
                self.on_while_until(
                    ctx,
                    w.keyword_loc().span(),
                    w.predicate().span(),
                    w.statements().map(|s| s.as_node()),
                    base,
                );
            }
            Node::UntilNode { .. } => {
                let u = rhs.as_until_node().expect("kind matched");
                self.on_while_until(
                    ctx,
                    u.keyword_loc().span(),
                    u.predicate().span(),
                    u.statements().map(|s| s.as_node()),
                    base,
                );
            }
            _ => unreachable!(),
        }
    }

    fn on_call(&mut self, ctx: &mut Context<'_>, node: &Node<'_>) {
        let call = node.as_call_node().expect("kind matched");

        // RuboCop's `adjacent_def_modifier?`: `(send nil? _ (any_def ...))`. RuboCop's
        // `leftmost_modifier_of` walks up while the parent is itself a bare send; we do not
        // track parent pointers, so instead we walk *down* from the outermost call of such a
        // chain (the traversal visits it first) through nested single-bare-argument calls
        // until we reach the `def`, treating the outermost call as the modifier base and
        // marking every node along the way as handled so the traversal does not re-process
        // the same chain from an inner call's perspective.
        if !self.ignored.contains(&node.span().start) && call.receiver().is_none() {
            if let Some(arg) = bare_single_argument(&call) {
                if let Some((def, chain)) = def_through_bare_chain(arg) {
                    let base = if self.def_end_alignment_is_def {
                        def.as_node().span()
                    } else {
                        node.span()
                    };
                    self.check_indentation(ctx, base, def.body(), "normal");
                    self.ignored.insert(def.as_node().span().start);
                    for link in chain {
                        self.ignored.insert(link);
                    }
                }
            }
        }

        // RuboCop's `CheckAssignment#on_send` (attribute/element writers: `foo.bar = ...`,
        // `foo[bar] = ...`).
        if call.is_attribute_write() {
            if let Some(args) = call.arguments() {
                if let Some(last) = args.arguments().last() {
                    self.check_assignment(ctx, node.span(), last);
                }
            }
        }

        // RuboCop's `on_block` (Prism has no parent pointer from a block back to the call
        // that owns it, so this fires from the owning `CallNode` instead, where the
        // receiver/dot locations needed for `dot_on_new_line?` are available).
        if let Some(block_node) = call.block() {
            if let Node::BlockNode { .. } = &block_node {
                let block = block_node.as_block_node().expect("kind matched");
                let end_span = block.closing_loc().span();
                if begins_its_line(ctx, end_span) {
                    let dot_on_new_line = call.call_operator_loc().is_some_and(|dot| {
                        call.receiver().is_some_and(|r| {
                            let last = r.span().end.saturating_sub(1).max(r.span().start);
                            ctx.line_col(last).line < ctx.line_col(dot.span().start).line
                        })
                    });
                    let base = if dot_on_new_line {
                        call.call_operator_loc().expect("checked above").span()
                    } else {
                        end_span
                    };
                    self.check_indentation(ctx, base, block.body(), "normal");
                    if self.indented_internal_methods {
                        self.check_members(ctx, end_span, block.body());
                    }
                }
            }
        }
    }
}

/// RuboCop's `starts_with_access_modifier?`.
fn starts_with_access_modifier(body: &Node<'_>) -> bool {
    if let Node::StatementsNode { .. } = body {
        let statements = body.as_statements_node().expect("kind matched");
        if let Some(first) = statements.body().first() {
            return is_bare_access_modifier(&first);
        }
    }
    false
}

/// The sole argument of a receiver-less call with exactly one positional argument, if any
/// (`(send nil? _ ARG)`).
fn bare_single_argument<'pr>(call: &ruby_ast::node::CallNode<'pr>) -> Option<Node<'pr>> {
    let args = call.arguments()?;
    let list = args.arguments();
    if list.len() == 1 {
        list.first()
    } else {
        None
    }
}

/// Walks down through a chain of receiver-less, single-bare-argument calls
/// (`foo bar def baz; end`) until it reaches the trailing `def`/`defs`, returning it together
/// with the start offsets of every intermediate call node passed through (which must be
/// marked handled so the traversal does not re-process the chain from an inner link).
fn def_through_bare_chain(mut node: Node<'_>) -> Option<(ruby_ast::node::DefNode<'_>, Vec<u32>)> {
    let mut chain = Vec::new();
    loop {
        match &node {
            Node::DefNode { .. } => {
                return Some((node.as_def_node().expect("kind matched"), chain))
            }
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("kind matched");
                if call.receiver().is_some() {
                    return None;
                }
                let start = node.span().start;
                let next = bare_single_argument(&call)?;
                chain.push(start);
                node = next;
            }
            _ => return None,
        }
    }
}

/// Extracts the assigned value from any of the local/instance/class/global-variable or
/// constant write-node kinds this rule subscribes to (RuboCop's `on_lvasgn` and its aliases).
fn write_node_value<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    match node {
        Node::LocalVariableWriteNode { .. } => {
            Some(node.as_local_variable_write_node().expect("kind matched").value())
        }
        Node::LocalVariableOperatorWriteNode { .. } => {
            Some(node.as_local_variable_operator_write_node().expect("kind matched").value())
        }
        Node::LocalVariableAndWriteNode { .. } => {
            Some(node.as_local_variable_and_write_node().expect("kind matched").value())
        }
        Node::LocalVariableOrWriteNode { .. } => {
            Some(node.as_local_variable_or_write_node().expect("kind matched").value())
        }
        Node::InstanceVariableWriteNode { .. } => {
            Some(node.as_instance_variable_write_node().expect("kind matched").value())
        }
        Node::InstanceVariableOperatorWriteNode { .. } => {
            Some(node.as_instance_variable_operator_write_node().expect("kind matched").value())
        }
        Node::InstanceVariableAndWriteNode { .. } => {
            Some(node.as_instance_variable_and_write_node().expect("kind matched").value())
        }
        Node::InstanceVariableOrWriteNode { .. } => {
            Some(node.as_instance_variable_or_write_node().expect("kind matched").value())
        }
        Node::ClassVariableWriteNode { .. } => {
            Some(node.as_class_variable_write_node().expect("kind matched").value())
        }
        Node::ClassVariableOperatorWriteNode { .. } => {
            Some(node.as_class_variable_operator_write_node().expect("kind matched").value())
        }
        Node::ClassVariableAndWriteNode { .. } => {
            Some(node.as_class_variable_and_write_node().expect("kind matched").value())
        }
        Node::ClassVariableOrWriteNode { .. } => {
            Some(node.as_class_variable_or_write_node().expect("kind matched").value())
        }
        Node::GlobalVariableWriteNode { .. } => {
            Some(node.as_global_variable_write_node().expect("kind matched").value())
        }
        Node::GlobalVariableOperatorWriteNode { .. } => {
            Some(node.as_global_variable_operator_write_node().expect("kind matched").value())
        }
        Node::GlobalVariableAndWriteNode { .. } => {
            Some(node.as_global_variable_and_write_node().expect("kind matched").value())
        }
        Node::GlobalVariableOrWriteNode { .. } => {
            Some(node.as_global_variable_or_write_node().expect("kind matched").value())
        }
        Node::ConstantWriteNode { .. } => {
            Some(node.as_constant_write_node().expect("kind matched").value())
        }
        Node::ConstantOperatorWriteNode { .. } => {
            Some(node.as_constant_operator_write_node().expect("kind matched").value())
        }
        Node::ConstantAndWriteNode { .. } => {
            Some(node.as_constant_and_write_node().expect("kind matched").value())
        }
        Node::ConstantOrWriteNode { .. } => {
            Some(node.as_constant_or_write_node().expect("kind matched").value())
        }
        _ => None,
    }
}

/// RuboCop's `bare_access_modifier?`/`access_modifier?`: a receiver-less, argument-less call to
/// `private`/`protected`/`public`/`module_function`.
fn is_bare_access_modifier(node: &Node<'_>) -> bool {
    if let Node::CallNode { .. } = node {
        let call = node.as_call_node().expect("kind matched");
        if call.receiver().is_some() {
            return false;
        }
        if call.arguments().is_some() {
            return false;
        }
        matches!(call.name().as_slice(), b"private" | b"protected" | b"public" | b"module_function")
    } else {
        false
    }
}

/// RuboCop's `special_modifier?`: a bare `private`/`protected` call (not `public`/
/// `module_function`).
fn is_special_modifier(node: &Node<'_>) -> bool {
    if let Node::CallNode { .. } = node {
        let call = node.as_call_node().expect("kind matched");
        if call.receiver().is_some() || call.arguments().is_some() {
            return false;
        }
        matches!(call.name().as_slice(), b"private" | b"protected")
    } else {
        false
    }
}

/// RuboCop's `offense`'s reassignment: a multi-statement, unwrapped body is narrowed to just
/// its first statement before autocorrection (this cop only fixes the first statement of a
/// body).
fn narrow_for_fix(body: Node<'_>) -> Node<'_> {
    if let Node::StatementsNode { .. } = body {
        let statements = body.as_statements_node().expect("kind matched");
        if statements.body().len() > 1 {
            if let Some(first) = statements.body().first() {
                return first;
            }
        }
    }
    body
}

/// RuboCop's `offending_range`.
fn offending_range(body_start: u32, indentation: i64) -> Span {
    if indentation >= 0 {
        let delta = u32::try_from(indentation).unwrap_or(u32::MAX);
        let ind = body_start.saturating_sub(delta);
        Span::new(ind, body_start)
    } else {
        let delta = u32::try_from(-indentation).unwrap_or(u32::MAX);
        Span::new(body_start, body_start + delta)
    }
}

/// Collects the byte ranges of heredoc bodies within a subtree, so autocorrection never
/// touches lines that are really heredoc content (RuboCop's `AlignmentCorrector`
/// `inside_string_ranges`/`inside_string_range`, heredoc case).
struct HeredocTaboo {
    ranges: Vec<Span>,
}

impl<'pr> Visitor<'pr> for HeredocTaboo {
    fn enter(&mut self, node: &Node<'pr>) {
        let opening_closing = match node {
            Node::StringNode { .. } => {
                let n = node.as_string_node().expect("kind matched");
                n.opening_loc().zip(n.closing_loc())
            }
            Node::InterpolatedStringNode { .. } => {
                let n = node.as_interpolated_string_node().expect("kind matched");
                n.opening_loc().zip(n.closing_loc())
            }
            Node::XStringNode { .. } => {
                let n = node.as_x_string_node().expect("kind matched");
                Some((n.opening_loc(), n.closing_loc()))
            }
            Node::InterpolatedXStringNode { .. } => {
                let n = node.as_interpolated_x_string_node().expect("kind matched");
                Some((n.opening_loc(), n.closing_loc()))
            }
            _ => None,
        };
        if let Some((open, close)) = opening_closing {
            if open.as_slice().starts_with(b"<<") {
                let close_span = close.span();
                self.ranges.push(Span::new(open.span().end, close_span.start));
            }
        }
    }
}

/// Builds the edits that shift every physical line of `target` by `column_delta` columns
/// (RuboCop's `AlignmentCorrector.correct`). Returns `None` when the whole correction must be
/// skipped (a `=begin`/`=end` block comment inside the range).
fn build_alignment_edits(
    ctx: &Context<'_>,
    target: &Node<'_>,
    column_delta: i64,
) -> Option<Vec<Edit>> {
    if column_delta == 0 {
        return None;
    }
    let span = target.span();
    let start_line = ctx.line_col(span.start).line;
    let last_byte = span.end.saturating_sub(1).max(span.start);
    let end_line = ctx.line_col(last_byte).line;

    for line in start_line..=end_line {
        let text = ctx.line_text(line);
        let trimmed = trim_start(text);
        if trimmed.starts_with(b"=begin") {
            return None;
        }
    }

    let mut taboo = HeredocTaboo { ranges: Vec::new() };
    walk(target, &mut taboo);

    let mut edits = Vec::new();
    for line in start_line..=end_line {
        let line_span = ctx.line_span(line);
        let is_first = line == start_line;
        let anchor = if is_first { span.start } else { line_span.start };

        if column_delta > 0 {
            if !is_first && line_span.is_empty() {
                continue;
            }
            if taboo.ranges.iter().any(|t| t.contains(Span::empty(anchor))) {
                continue;
            }
            let width = usize::try_from(column_delta).unwrap_or(0);
            edits.push(Edit::insert(anchor, " ".repeat(width).into_bytes()));
        } else {
            let n = u32::try_from(-column_delta).unwrap_or(0);
            let starts_with_space = ctx
                .source()
                .bytes()
                .get(usize::try_from(anchor).unwrap_or(usize::MAX))
                .is_some_and(|&b| b == b' ');
            let range = if is_first {
                Span::new(anchor.saturating_sub(n), anchor)
            } else if starts_with_space {
                Span::new(anchor, anchor + n)
            } else {
                Span::new(anchor.saturating_sub(n), anchor)
            };
            if taboo.ranges.iter().any(|t| t.contains(range)) {
                continue;
            }
            let text = ctx.text(range);
            if !text.is_empty() && text.iter().all(|&b| b == b' ' || b == b'\t') {
                edits.push(Edit::delete(range));
            }
        }
    }
    if edits.is_empty() {
        None
    } else {
        Some(edits)
    }
}

fn trim_start(text: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < text.len() && text[i].is_ascii_whitespace() {
        i += 1;
    }
    &text[i..]
}

impl Rule for IndentationWidth {
    const META: RuleMeta = RuleMeta {
        name: "Layout/IndentationWidth",
        department: Department::Layout,
        summary: "Checks for indentation that doesn't use the specified number of spaces.",
        explanation: "\
The indentation width can be configured using the `Width` setting. The default width is 2.

See also the `Layout/IndentationConsistency` cop which is the companion to this one.

```ruby
# bad
class A
 def test
  puts 'hello'
 end
end

# good
class A
  def test
    puts 'hello'
  end
end
```

Lines that match `AllowedPatterns` are not required to follow the configured width:

```ruby
# AllowedPatterns: ['^\\s*module']

# bad
module A
class B
  def test
  puts 'hello'
  end
end
end

# good
module A
class B
  def test
    puts 'hello'
  end
end
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::DefNode,
            NodeKind::ClassNode,
            NodeKind::ModuleNode,
            NodeKind::SingletonClassNode,
            NodeKind::IfNode,
            NodeKind::UnlessNode,
            NodeKind::WhileNode,
            NodeKind::UntilNode,
            NodeKind::CaseNode,
            NodeKind::CaseMatchNode,
            NodeKind::LambdaNode,
            NodeKind::BeginNode,
            NodeKind::RescueNode,
            NodeKind::EnsureNode,
            NodeKind::ForNode,
            NodeKind::CallNode,
            NodeKind::LocalVariableWriteNode,
            NodeKind::LocalVariableOperatorWriteNode,
            NodeKind::LocalVariableAndWriteNode,
            NodeKind::LocalVariableOrWriteNode,
            NodeKind::InstanceVariableWriteNode,
            NodeKind::InstanceVariableOperatorWriteNode,
            NodeKind::InstanceVariableAndWriteNode,
            NodeKind::InstanceVariableOrWriteNode,
            NodeKind::ClassVariableWriteNode,
            NodeKind::ClassVariableOperatorWriteNode,
            NodeKind::ClassVariableAndWriteNode,
            NodeKind::ClassVariableOrWriteNode,
            NodeKind::GlobalVariableWriteNode,
            NodeKind::GlobalVariableOperatorWriteNode,
            NodeKind::GlobalVariableAndWriteNode,
            NodeKind::GlobalVariableOrWriteNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantOperatorWriteNode,
            NodeKind::ConstantAndWriteNode,
            NodeKind::ConstantOrWriteNode,
        ],
        config: &[
            ConfigOption {
                name: "Width",
                default: ConfigDefault::Int(2),
                allowed: &[],
                doc: "Number of spaces for each indentation level.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Lines matching one of these patterns are not required to follow the configured width.",
            },
        ],
        blind_spots: "\
Does not track parent pointers, so `leftmost_modifier_of` (chained bare-modifier calls such as
`foo private def bar; end`) approximates with the innermost call's own location; only a single
level of `modifier def` nesting is verified against fixtures. `first_part_of_call_chain` (a
conditional assigned through a trailing method/block chain, e.g. `var = if a; 1; end.freeze`) is
not unwound; only a bare conditional value is recognized. `Layout/AccessModifierIndentation`'s
`macro?`/`in_macro_scope?` nuance is approximated by a simple bare-call-name check (no scope
verification). Autocorrection does not special-case parenthesized multi-statement bodies (RuboCop's
`parentheses?` guard); it always narrows to the first statement. Non-heredoc multi-line string/
symbol literals are not added to the autocorrect taboo ranges (only heredoc bodies are), so a
reindented statement that embeds a multi-line plain string could shift that string's continuation
lines; this has not triggered in the fixture set.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let allowed_patterns = options
            .str_list("AllowedPatterns")
            .into_iter()
            .filter_map(|p| regex::Regex::new(&p).ok())
            .collect();

        let indented_internal_methods = options
            .peer("Layout/IndentationConsistency", "EnforcedStyle")
            .and_then(|v| v.as_str())
            .unwrap_or(CONSISTENCY_STYLE_DEFAULT)
            == "indented_internal_methods";

        let access_modifier_outdent = options
            .peer("Layout/AccessModifierIndentation", "EnforcedStyle")
            .and_then(|v| v.as_str())
            .unwrap_or(ACCESS_MODIFIER_STYLE_DEFAULT)
            == "outdent";

        let end_alignment = match options
            .peer("Layout/EndAlignment", "EnforcedStyleAlignWith")
            .and_then(|v| v.as_str())
            .unwrap_or(END_ALIGNMENT_DEFAULT)
        {
            "variable" => EndAlignment::Variable,
            "start_of_line" => EndAlignment::StartOfLine,
            _ => EndAlignment::Keyword,
        };

        let def_end_alignment_is_def = options
            .peer("Layout/DefEndAlignment", "EnforcedStyleAlignWith")
            .and_then(|v| v.as_str())
            .unwrap_or(DEF_END_ALIGNMENT_DEFAULT)
            == "def";

        Ok(Self {
            width: options.int("Width"),
            allowed_patterns,
            indented_internal_methods,
            access_modifier_outdent,
            end_alignment,
            def_end_alignment_is_def,
            offense_ranges: Vec::new(),
            ignored: HashSet::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.offense_ranges.clear();
        self.ignored.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::DefNode { .. } => self.on_def(ctx, node),
            Node::ClassNode { .. } => {
                let class = node.as_class_node().expect("kind matched");
                self.on_class_like(
                    ctx,
                    class.location().span(),
                    class.class_keyword_loc().span(),
                    class.body(),
                );
            }
            Node::ModuleNode { .. } => {
                let module = node.as_module_node().expect("kind matched");
                self.on_class_like(
                    ctx,
                    module.location().span(),
                    module.module_keyword_loc().span(),
                    module.body(),
                );
            }
            Node::SingletonClassNode { .. } => {
                let sclass = node.as_singleton_class_node().expect("kind matched");
                self.on_class_like(
                    ctx,
                    sclass.location().span(),
                    sclass.class_keyword_loc().span(),
                    sclass.body(),
                );
            }
            Node::IfNode { .. } => {
                if self.ignored.contains(&node.span().start) {
                    return;
                }
                self.on_if(ctx, node, node.span());
            }
            Node::UnlessNode { .. } => {
                if self.ignored.contains(&node.span().start) {
                    return;
                }
                self.on_unless(ctx, node);
            }
            Node::WhileNode { .. } => self.on_while_node(ctx, node),
            Node::UntilNode { .. } => self.on_until_node(ctx, node),
            Node::CaseNode { .. } => self.on_case(ctx, node),
            Node::CaseMatchNode { .. } => self.on_case_match(ctx, node),
            Node::LambdaNode { .. } => self.on_lambda(ctx, node),
            Node::BeginNode { .. } => self.on_begin(ctx, node),
            Node::RescueNode { .. } => self.on_rescue(ctx, node),
            Node::EnsureNode { .. } => self.on_ensure(ctx, node),
            Node::ForNode { .. } => self.on_for(ctx, node),
            Node::CallNode { .. } => self.on_call(ctx, node),
            Node::LocalVariableWriteNode { .. }
            | Node::LocalVariableOperatorWriteNode { .. }
            | Node::LocalVariableAndWriteNode { .. }
            | Node::LocalVariableOrWriteNode { .. }
            | Node::InstanceVariableWriteNode { .. }
            | Node::InstanceVariableOperatorWriteNode { .. }
            | Node::InstanceVariableAndWriteNode { .. }
            | Node::InstanceVariableOrWriteNode { .. }
            | Node::ClassVariableWriteNode { .. }
            | Node::ClassVariableOperatorWriteNode { .. }
            | Node::ClassVariableAndWriteNode { .. }
            | Node::ClassVariableOrWriteNode { .. }
            | Node::GlobalVariableWriteNode { .. }
            | Node::GlobalVariableOperatorWriteNode { .. }
            | Node::GlobalVariableAndWriteNode { .. }
            | Node::GlobalVariableOrWriteNode { .. }
            | Node::ConstantWriteNode { .. }
            | Node::ConstantOperatorWriteNode { .. }
            | Node::ConstantAndWriteNode { .. }
            | Node::ConstantOrWriteNode { .. } => {
                if let Some(value) = write_node_value(node) {
                    self.check_assignment(ctx, node.span(), value);
                }
            }
            _ => {}
        }
    }
}
