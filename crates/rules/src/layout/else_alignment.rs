//! `Layout/ElseAlignment`, ported from RuboCop's
//! `lib/rubocop/cop/layout/else_alignment.rb` plus the `EndKeywordAlignment`,
//! `Alignment` and `CheckAssignment` mixins it includes.
//!
//! Prism differences this port bridges:
//!
//! * Whitequark's `rescue` node is Prism's [`NodeKind::BeginNode`] with both
//!   a rescue and an else clause; an explicit `begin ... end` is the same
//!   node with a `begin` keyword, and an `ensure` clause is a sibling rather
//!   than a wrapper, so `base_range_of_rescue`'s `parent.parent if
//!   parent.ensure_type?` hop is already folded in.
//! * A whitequark `block` node's source range starts at its receiver;
//!   Prism's `BlockNode` starts at `do`/`{`, so `start_line_range` is taken
//!   from the enclosing call.
//! * An `elsif` is an `IfNode` whose keyword is `elsif`, and a ternary is an
//!   `IfNode` with no keyword at all but a `:`-flavoured `ElseNode`; neither
//!   has a whitequark `loc.else` of its own shape, so both are special-cased.

use std::collections::HashSet;

use linter::{
    Applicability, Context, Department, Fix, FixAvailability, NodeInfo, OptionError, OptionValue,
    Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::end_keyword_alignment::{
    column_offset_between, extract_rhs, first_part_of_call_chain, start_line_range,
};

/// Align elses and elsifs correctly.
#[derive(Debug, Clone)]
pub struct ElseAlignment {
    /// `Layout/EndAlignment`'s `EnforcedStyleAlignWith` is anything but
    /// `keyword`, which is what `variable_alignment?` keys off.
    end_alignment_by_variable: bool,
    /// RuboCop's `ignore_node`, for `if`s already checked with an explicit
    /// base (an assignment's right-hand side, or an `elsif` link).
    ignored: HashSet<u32>,
    /// Selector span of every open `CallNode` ancestor, outermost first --
    /// `base_for_method_definition`'s `parent.loc.selector`.
    call_selectors: Vec<Option<Span>>,
    /// Keyword span of every open `if`/`unless` ancestor that is neither an
    /// `elsif` link nor a ternary, outermost first -- the lineage
    /// `base_range_of_if` searches.
    if_keywords: Vec<Option<Span>>,
}

impl Rule for ElseAlignment {
    const META: RuleMeta = RuleMeta {
        name: "Layout/ElseAlignment",
        department: Department::Layout,
        summary: "Align elses and elsifs correctly.",
        explanation: "\
`else` and `elsif` normally line up with the `if`/`unless`/`while`/`until`/
`begin`/`def`/`rescue` keyword they belong to; in an assignment they follow
`Layout/EndAlignment`'s `EnforcedStyleAlignWith` instead.

```ruby
# bad
if something
  code
 else
  code
end

# good
if something
  code
else
  code
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: KINDS,
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let end_style = options
            .peer("Layout/EndAlignment", "EnforcedStyleAlignWith")
            .and_then(OptionValue::as_str)
            .unwrap_or("keyword")
            .to_owned();
        Ok(Self {
            end_alignment_by_variable: end_style != "keyword",
            ignored: HashSet::new(),
            call_selectors: Vec::new(),
            if_keywords: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("kind matched");
                // `CheckAssignment#on_send` is not aliased to `on_csend`.
                if !call.is_safe_navigation() {
                    self.check_assignment(ctx, node);
                }
                self.call_selectors.push(call.message_loc().map(|l| l.span()));
            }
            Node::IfNode { .. } | Node::UnlessNode { .. } => {
                self.handle_if(ctx, node, None);
                self.if_keywords.push(base_keyword_of(node));
            }
            Node::CaseNode { .. } => {
                let case = node.as_case_node().expect("kind matched");
                let Some(else_clause) = case.else_clause() else { return };
                let Some(last) = case.conditions().iter().last() else { return };
                let Some(when) = last.as_when_node() else { return };
                Self::check_alignment(
                    ctx,
                    when.keyword_loc().span(),
                    else_clause.else_keyword_loc().span(),
                );
            }
            Node::CaseMatchNode { .. } => {
                let case = node.as_case_match_node().expect("kind matched");
                let Some(else_clause) = case.else_clause() else { return };
                let Some(last) = case.conditions().iter().last() else { return };
                let Some(in_node) = last.as_in_node() else { return };
                Self::check_alignment(
                    ctx,
                    in_node.in_loc().span(),
                    else_clause.else_keyword_loc().span(),
                );
            }
            Node::BeginNode { .. } => {
                let begin = node.as_begin_node().expect("kind matched");
                let Some(rescue) = begin.rescue_clause() else { return };
                let Some(else_clause) = begin.else_clause() else { return };
                let base = self.base_range_of_rescue(
                    ctx,
                    begin.begin_keyword_loc().map(|l| l.span()),
                    rescue.keyword_loc().span(),
                );
                Self::check_alignment(ctx, base, else_clause.else_keyword_loc().span());
            }
            _ => self.check_assignment(ctx, node),
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        match node {
            Node::CallNode { .. } => {
                self.call_selectors.pop();
            }
            Node::IfNode { .. } | Node::UnlessNode { .. } => {
                self.if_keywords.pop();
            }
            _ => {}
        }
    }
}

impl ElseAlignment {
    /// RuboCop's `on_if`, with its `check_nested` recursion over an `elsif`
    /// chain unrolled: every link resolves the same `base_range_of_if`, so
    /// it is computed once.
    fn handle_if(&mut self, ctx: &mut Context<'_>, node: &Node<'_>, base: Option<Span>) {
        if self.ignored.contains(&node.span().start) {
            return;
        }
        let base_range = match base {
            Some(base) => base,
            None => self.lineage_keyword(node),
        };
        let mut current = *node;
        loop {
            let Some(else_loc) = else_keyword_of(&current) else { return };
            if !ctx.begins_its_line(else_loc) {
                return;
            }
            Self::check_alignment(ctx, base_range, else_loc);
            let Some(elsif) = elsif_branch(&current) else { return };
            self.ignored.insert(elsif.span().start);
            current = elsif;
        }
    }

    /// RuboCop's `base_range_of_if` with no explicit base: the keyword of
    /// the nearest enclosing real `if`/`unless`, starting with the node
    /// itself.
    fn lineage_keyword(&self, node: &Node<'_>) -> Span {
        if let Some(keyword) = base_keyword_of(node) {
            return keyword;
        }
        self.if_keywords
            .iter()
            .rev()
            .find_map(|keyword| *keyword)
            .unwrap_or_else(|| Span::new(node.span().start, node.span().start))
    }

    /// RuboCop's `base_range_of_rescue`.
    fn base_range_of_rescue(
        &self,
        ctx: &Context<'_>,
        begin_keyword: Option<Span>,
        rescue_keyword: Span,
    ) -> Span {
        // `when :kwbegin then parent.loc.begin`.
        if let Some(begin_keyword) = begin_keyword {
            return begin_keyword;
        }
        let ancestors = ctx.ancestors();
        let Some(parent) = ancestors.last() else { return rescue_keyword };
        match parent.kind {
            NodeKind::DefNode => self.base_for_method_definition(ctx, parent.span),
            NodeKind::BlockNode => {
                // A whitequark `block` node starts at its receiver.
                let call = ancestors
                    .len()
                    .checked_sub(2)
                    .and_then(|i| ancestors.get(i))
                    .filter(|info| info.kind == NodeKind::CallNode)
                    .map_or(parent.span, |info| info.span);
                start_line_range(ctx, call)
            }
            NodeKind::LambdaNode => start_line_range(ctx, parent.span),
            NodeKind::ClassNode | NodeKind::SingletonClassNode => {
                Span::new(parent.span.start, parent.span.start + 5)
            }
            NodeKind::ModuleNode => Span::new(parent.span.start, parent.span.start + 6),
            _ => rescue_keyword,
        }
    }

    /// RuboCop's `base_for_method_definition`: the `private`/`public`
    /// selector for `private def ...`, else the `def` keyword.
    fn base_for_method_definition(&self, ctx: &Context<'_>, def_span: Span) -> Span {
        let ancestors = ctx.ancestors();
        let grandparent =
            ancestors.len().checked_sub(2).and_then(|i| ancestors.get(i)).copied().and_then(
                |info: NodeInfo| {
                    if info.kind == NodeKind::ArgumentsNode {
                        ancestors.len().checked_sub(3).and_then(|i| ancestors.get(i)).copied()
                    } else {
                        Some(info)
                    }
                },
            );
        if grandparent.is_some_and(|info| info.kind == NodeKind::CallNode) {
            if let Some(Some(selector)) = self.call_selectors.last() {
                return *selector;
            }
        }
        Span::new(def_span.start, def_span.start + 3)
    }

    /// RuboCop's `check_assignment`, reached from `CheckAssignment`'s
    /// `on_send` and assignment callbacks.
    fn check_assignment(&mut self, ctx: &mut Context<'_>, node: &Node<'_>) {
        let Some(rhs) = extract_rhs(node) else { return };
        let Some(rhs) = first_part_of_call_chain(rhs, |_| {}) else { return };
        if !matches!(rhs, Node::IfNode { .. } | Node::UnlessNode { .. }) {
            return;
        }
        let line_break = ctx.line_col(rhs.span().start).line > ctx.line_col(node.span().start).line;
        let base =
            if self.end_alignment_by_variable && !line_break { node.span() } else { rhs.span() };
        self.handle_if(ctx, &rhs, Some(base));
        self.ignored.insert(rhs.span().start);
    }

    /// RuboCop's `check_alignment` plus `autocorrect`.
    fn check_alignment(ctx: &mut Context<'_>, base_range: Span, else_range: Span) {
        if !ctx.begins_its_line(else_range) {
            return;
        }
        let delta = column_offset_between(ctx, base_range, else_range);
        if delta == 0 {
            return;
        }
        let else_source = String::from_utf8_lossy(ctx.text(else_range));
        let base_text = ctx.text(base_range);
        let word_end =
            base_text.iter().position(u8::is_ascii_whitespace).unwrap_or(base_text.len());
        let base_word = String::from_utf8_lossy(&base_text[..word_end]);
        let message = format!("Align `{else_source}` with `{base_word}`.");
        let edits = linter::shift_lines(ctx, else_range, i32::try_from(delta).unwrap_or(0), &[]);
        if edits.is_empty() {
            ctx.report(&Self::META, else_range, message);
        } else {
            ctx.report_with_fix(
                &Self::META,
                else_range,
                message,
                Fix { applicability: Applicability::Safe, edits },
            );
        }
    }
}

/// `if?`/`unless?`: the keyword span of a real `if`/`unless`, and nothing
/// for an `elsif` link or a ternary.
fn base_keyword_of(node: &Node<'_>) -> Option<Span> {
    match node {
        Node::UnlessNode { .. } => {
            Some(node.as_unless_node().expect("kind matched").keyword_loc().span())
        }
        Node::IfNode { .. } => {
            let keyword = node.as_if_node().expect("kind matched").if_keyword_loc()?;
            if keyword.as_slice() == b"if" {
                Some(keyword.span())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Whitequark's `loc.else` for an `if`/`unless`: the `else` keyword, or the
/// `elsif` keyword when the node chains into one. A ternary's `:` is not
/// one (whitequark maps it to `loc.colon`).
fn else_keyword_of(node: &Node<'_>) -> Option<Span> {
    match node {
        Node::UnlessNode { .. } => node
            .as_unless_node()
            .expect("kind matched")
            .else_clause()
            .map(|e| e.else_keyword_loc().span()),
        Node::IfNode { .. } => {
            let if_node = node.as_if_node().expect("kind matched");
            if_node.if_keyword_loc()?;
            let subsequent = if_node.subsequent()?;
            match &subsequent {
                Node::ElseNode { .. } => {
                    subsequent.as_else_node().map(|else_node| else_node.else_keyword_loc().span())
                }
                _ => subsequent.as_if_node().and_then(|i| i.if_keyword_loc()).map(|l| l.span()),
            }
        }
        _ => None,
    }
}

/// `elsif_conditional?`: the `elsif` link this node chains into.
fn elsif_branch<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    let if_node = node.as_if_node()?;
    let subsequent = if_node.subsequent()?;
    match subsequent {
        Node::IfNode { .. } => Some(subsequent),
        _ => None,
    }
}

/// The subscribed kinds: the keyword constructs that can own an `else`,
/// plus `CheckAssignment`'s assignment and call nodes.
const KINDS: &[NodeKind] = &[
    NodeKind::IfNode,
    NodeKind::UnlessNode,
    NodeKind::CaseNode,
    NodeKind::CaseMatchNode,
    NodeKind::BeginNode,
    NodeKind::CallNode,
    NodeKind::LocalVariableWriteNode,
    NodeKind::InstanceVariableWriteNode,
    NodeKind::ClassVariableWriteNode,
    NodeKind::GlobalVariableWriteNode,
    NodeKind::ConstantWriteNode,
    NodeKind::ConstantPathWriteNode,
    NodeKind::MultiWriteNode,
    NodeKind::LocalVariableAndWriteNode,
    NodeKind::LocalVariableOrWriteNode,
    NodeKind::LocalVariableOperatorWriteNode,
    NodeKind::InstanceVariableAndWriteNode,
    NodeKind::InstanceVariableOrWriteNode,
    NodeKind::InstanceVariableOperatorWriteNode,
    NodeKind::ClassVariableAndWriteNode,
    NodeKind::ClassVariableOrWriteNode,
    NodeKind::ClassVariableOperatorWriteNode,
    NodeKind::GlobalVariableAndWriteNode,
    NodeKind::GlobalVariableOrWriteNode,
    NodeKind::GlobalVariableOperatorWriteNode,
    NodeKind::ConstantAndWriteNode,
    NodeKind::ConstantOrWriteNode,
    NodeKind::ConstantOperatorWriteNode,
    NodeKind::ConstantPathAndWriteNode,
    NodeKind::ConstantPathOrWriteNode,
    NodeKind::ConstantPathOperatorWriteNode,
    NodeKind::CallAndWriteNode,
    NodeKind::CallOrWriteNode,
    NodeKind::CallOperatorWriteNode,
    NodeKind::IndexAndWriteNode,
    NodeKind::IndexOrWriteNode,
    NodeKind::IndexOperatorWriteNode,
];
