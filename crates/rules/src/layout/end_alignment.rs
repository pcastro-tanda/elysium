//! `Layout/EndAlignment`, ported from RuboCop's
//! `lib/rubocop/cop/layout/end_alignment.rb` plus the `EndKeywordAlignment`
//! and `CheckAssignment` mixins it includes.
//!
//! Only the configured style's alignment range is built: upstream assembles
//! all three and then reads `align_ranges[style]` plus
//! `matching.key?(style)`; the other entries only feed `style_detected`,
//! which drives `--auto-gen-config` and nothing else.
//!
//! `EnforcedStyleAlignWith: variable` needs the conditional's ancestors,
//! which Prism's kind+span ancestor stack cannot fully answer (whether a
//! `CallNode` ancestor is an operator method). Facts about each open call
//! are therefore recorded on `enter` and popped on `leave`, and the
//! receiver chain walked down by `check_assignment` is appended to them, so
//! the chain this cop sees is exactly `node.ancestors` upstream.

use std::collections::HashSet;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::end_keyword_alignment::{
    align_end, end_is_aligned, extract_rhs, first_part_of_call_chain, is_assignment_kind,
    is_setter_call, logical_parent, misalignment_message, start_line_range, uses_tabs,
};
use super::first_argument_indentation::is_operator_method;

/// RuboCop's `EnforcedStyleAlignWith`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Keyword,
    Variable,
    StartOfLine,
}

/// One open `CallNode` ancestor, for the `variable` style's ancestor search.
#[derive(Debug, Clone, Copy)]
struct CallFact {
    is_operator: bool,
    is_safe_navigation: bool,
    /// `SendNode#assignment?`, which aliases `setter_method?`.
    is_setter: bool,
}

/// One entry of the conditional node's ancestor chain, nearest first.
#[derive(Debug, Clone, Copy)]
struct Anc {
    span: Span,
    is_send: bool,
    is_assignment: bool,
    is_operator: bool,
}

/// A keyword construct that owns an `end`.
#[derive(Debug, Clone, Copy)]
struct Construct {
    span: Span,
    keyword: Span,
    end: Option<Span>,
}

/// Align ends correctly.
#[derive(Debug, Clone)]
pub struct EndAlignment {
    style: Style,
    tabs: bool,
    /// RuboCop's `ignore_node`: conditionals an enclosing assignment has
    /// already checked.
    ignored: HashSet<u32>,
    /// `CallNode` ancestors currently open, outermost first.
    calls: Vec<CallFact>,
    /// Scratch: the receiver chain `check_assignment` descends, outermost
    /// first. Reused across nodes so the walk allocates once per file.
    path: Vec<Anc>,
    /// Scratch: the materialised ancestor chain, nearest first.
    chain: Vec<Anc>,
}

impl Rule for EndAlignment {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EndAlignment",
        department: Department::Layout,
        summary: "Align ends correctly.",
        explanation: "\
```ruby
# bad
variable = if true
    end

# good (keyword)
variable = if true
           end

# good (variable)
variable = if true
end

# good (start_of_line)
puts(if true
end)
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: KINDS,
        config: &[ConfigOption {
            name: "EnforcedStyleAlignWith",
            default: ConfigDefault::Str("keyword"),
            allowed: &["keyword", "variable", "start_of_line"],
            doc: "Whether `end` lines up with the matching keyword (`keyword`), with the \
                  left-hand side of an enclosing assignment (`variable`), or with the start \
                  of the line the keyword appears on (`start_of_line`).",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyleAlignWith")? {
            "variable" => Style::Variable,
            "start_of_line" => Style::StartOfLine,
            _ => Style::Keyword,
        };
        Ok(Self {
            style,
            tabs: uses_tabs(options),
            ignored: HashSet::new(),
            calls: Vec::new(),
            path: Vec::new(),
            chain: Vec::new(),
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
                self.calls.push(CallFact {
                    is_operator: is_operator_method(call.name().as_slice()),
                    is_safe_navigation: call.is_safe_navigation(),
                    is_setter: is_setter_call(&call),
                });
            }
            Node::SingletonClassNode { .. } => {
                let sclass = node.as_singleton_class_node().expect("kind matched");
                let construct = Construct {
                    span: node.span(),
                    keyword: sclass.class_keyword_loc().span(),
                    end: Some(sclass.end_keyword_loc().span()),
                };
                match self.assignment_parent(ctx) {
                    Some(parent) => {
                        self.path.clear();
                        self.check_asgn(ctx, &construct, parent, None);
                    }
                    None => self.check_other(ctx, &construct),
                }
            }
            Node::CaseNode { .. } | Node::CaseMatchNode { .. } => {
                let construct = construct_of(node);
                let Some(construct) = construct else { return };
                match self.argument_parent(ctx) {
                    Some(parent) => {
                        self.path.clear();
                        self.check_asgn(ctx, &construct, parent, Some(parent));
                    }
                    None => self.check_other(ctx, &construct),
                }
            }
            _ => {
                if let Some(construct) = construct_of(node) {
                    self.check_other(ctx, &construct);
                } else if is_assignment_kind(node.kind()) {
                    self.check_assignment(ctx, node);
                }
            }
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if let Node::CallNode { .. } = node {
            self.calls.pop();
        }
    }
}

impl EndAlignment {
    /// RuboCop's `check_other_alignment`.
    fn check_other(&mut self, ctx: &mut Context<'_>, construct: &Construct) {
        // The conditional is being entered directly, so its ancestors are
        // exactly the live stack; no receiver chain was descended.
        self.path.clear();
        let align_with = match self.style {
            Style::Keyword | Style::Variable => construct.keyword,
            Style::StartOfLine => start_line_range(ctx, construct.span),
        };
        self.check_end_kw(ctx, construct, align_with, None);
    }

    /// RuboCop's `check_asgn_alignment`. `outer` is the enclosing
    /// assignment or call; `case_parent` is set only on the `on_case`
    /// argument path, where `alignment_node_for_variable_style` has its own
    /// branch.
    fn check_asgn(
        &mut self,
        ctx: &mut Context<'_>,
        construct: &Construct,
        outer: Span,
        case_parent: Option<Span>,
    ) {
        let align_with = match self.style {
            Style::Keyword => construct.keyword,
            Style::StartOfLine => start_line_range(ctx, construct.span),
            Style::Variable => {
                // `line_break_before_keyword?(outer.source_range, inner)`.
                if ctx.line_col(construct.span.start).line > ctx.line_col(outer.start).line {
                    construct.keyword
                } else {
                    Span::new(outer.start, construct.keyword.end)
                }
            }
        };
        self.check_end_kw(ctx, construct, align_with, case_parent);
        self.ignored.insert(construct.span.start);
    }

    /// RuboCop's `EndKeywordAlignment#check_end_kw_alignment` plus
    /// `add_offense_for_misalignment` and `autocorrect`.
    fn check_end_kw(
        &mut self,
        ctx: &mut Context<'_>,
        construct: &Construct,
        align_with: Span,
        case_parent: Option<Span>,
    ) {
        if self.ignored.contains(&construct.span.start) {
            return;
        }
        let Some(end_loc) = construct.end else { return };
        if end_is_aligned(ctx, align_with, end_loc) {
            return;
        }
        let message = misalignment_message(ctx, end_loc, align_with);
        let column = self.alignment_column(ctx, construct, align_with, case_parent);
        match align_end(ctx, end_loc, column, self.tabs) {
            Some(edit) => ctx.report_with_fix(
                &Self::META,
                end_loc,
                message,
                Fix { applicability: Applicability::Safe, edits: vec![edit] },
            ),
            None => ctx.report(&Self::META, end_loc, message),
        }
    }

    /// RuboCop's `alignment_node` plus `AlignmentCorrector.alignment_column`.
    fn alignment_column(
        &mut self,
        ctx: &Context<'_>,
        construct: &Construct,
        align_with: Span,
        case_parent: Option<Span>,
    ) -> u32 {
        let target = match self.style {
            Style::Keyword => construct.span,
            // The `start_of_line` range is exactly `align_with` here.
            Style::StartOfLine => align_with,
            Style::Variable => self.variable_alignment_target(ctx, construct, case_parent),
        };
        ctx.line_col(target.start).column
    }

    /// RuboCop's `alignment_node_for_variable_style` plus the
    /// same-line-parent climb `alignment_node` wraps it in.
    fn variable_alignment_target(
        &mut self,
        ctx: &Context<'_>,
        construct: &Construct,
        case_parent: Option<Span>,
    ) -> Span {
        self.build_chain(ctx);
        let mut index: Option<usize> = None;
        let same_line_case_parent =
            case_parent.filter(|parent| ctx.same_line(construct.span, *parent));
        if same_line_case_parent.is_some() {
            index = Some(0);
        } else if let Some(found) =
            self.chain.iter().position(|a| a.is_assignment || (a.is_send && a.is_operator))
        {
            let assignment = self.chain[found];
            let line_break =
                ctx.line_col(construct.span.start).line > ctx.line_col(assignment.span.start).line;
            if !line_break {
                index = Some(found);
            }
        }
        loop {
            let next = index.map_or(0, |k| k + 1);
            let Some(parent) = self.chain.get(next).copied() else { break };
            if !parent.is_send {
                break;
            }
            let current = index.map_or(construct.span, |k| self.chain[k].span);
            if !ctx.same_line(current, parent.span) {
                break;
            }
            index = Some(next);
        }
        index.map_or(construct.span, |k| self.chain[k].span)
    }

    /// Materialises the conditional's ancestors, nearest first: the
    /// receiver chain `check_assignment` walked down (`self.path`) followed
    /// by the real ancestor stack, with Prism's `ArgumentsNode` wrapper
    /// skipped since whitequark has no such node.
    fn build_chain(&mut self, ctx: &Context<'_>) {
        self.chain.clear();
        self.chain.extend(self.path.iter().rev().copied());
        let mut call_index = self.calls.len();
        for info in ctx.ancestors().iter().rev() {
            if info.kind == NodeKind::ArgumentsNode {
                continue;
            }
            let is_send = info.kind == NodeKind::CallNode;
            let mut is_operator = false;
            let mut is_assignment = is_assignment_kind(info.kind);
            if is_send {
                call_index = call_index.saturating_sub(1);
                if let Some(fact) = self.calls.get(call_index) {
                    is_operator = fact.is_operator;
                    is_assignment = fact.is_setter;
                }
            }
            self.chain.push(Anc { span: info.span, is_send, is_assignment, is_operator });
        }
    }

    /// RuboCop's `check_assignment`, reached from `CheckAssignment`'s
    /// `on_send` and assignment callbacks.
    fn check_assignment(&mut self, ctx: &mut Context<'_>, node: &Node<'_>) {
        let Some(rhs) = extract_rhs(node) else { return };
        let self_anc = anc_of_node(node);
        self.path.clear();
        self.path.push(self_anc);
        let path = &mut self.path;
        let Some(mut rhs) = first_part_of_call_chain(rhs, |step| path.push(anc_of_node(step)))
        else {
            return;
        };
        // `rhs = rhs.child_nodes.first while rhs&.type?(:begin, :or, :and)`.
        loop {
            let next = match &rhs {
                Node::ParenthesesNode { .. } => {
                    let parens = rhs.as_parentheses_node().expect("kind matched");
                    match parens.body() {
                        Some(body) => match &body {
                            Node::StatementsNode { .. } => body
                                .as_statements_node()
                                .expect("kind matched")
                                .body()
                                .iter()
                                .next(),
                            _ => Some(body),
                        },
                        None => None,
                    }
                }
                Node::OrNode { .. } => Some(rhs.as_or_node().expect("kind matched").left()),
                Node::AndNode { .. } => Some(rhs.as_and_node().expect("kind matched").left()),
                _ => break,
            };
            self.path.push(anc_of_node(&rhs));
            match next {
                Some(next) => rhs = next,
                None => return,
            }
        }
        let Some(construct) = conditional_of(&rhs) else { return };
        self.check_asgn(ctx, &construct, node.span(), None);
    }

    /// `node.parent&.assignment?` for the node being entered, with
    /// `SendNode`'s `setter_method?` override honoured.
    fn assignment_parent(&self, ctx: &Context<'_>) -> Option<Span> {
        let parent = logical_parent(ctx)?;
        let is_assignment = if parent.kind == NodeKind::CallNode {
            self.calls.last().is_some_and(|c| c.is_setter)
        } else {
            is_assignment_kind(parent.kind)
        };
        is_assignment.then_some(parent.span)
    }

    /// `rubocop-ast`'s `Node#argument?` for the node being entered: its
    /// parent is a `send` whose arguments include it. Prism's
    /// `ArgumentsNode` wrapper stands between the two.
    fn argument_parent(&self, ctx: &Context<'_>) -> Option<Span> {
        let ancestors = ctx.ancestors();
        if ancestors.last()?.kind != NodeKind::ArgumentsNode {
            return None;
        }
        let parent = ancestors.get(ancestors.len().checked_sub(2)?)?;
        if parent.kind != NodeKind::CallNode {
            return None;
        }
        // Whitequark's `csend` is not `send_type?`.
        if self.calls.last().is_some_and(|c| c.is_safe_navigation) {
            return None;
        }
        Some(parent.span)
    }
}

fn anc_of_node(node: &Node<'_>) -> Anc {
    let kind = node.kind();
    let mut is_operator = false;
    let mut is_assignment = is_assignment_kind(kind);
    if kind == NodeKind::CallNode {
        let call = node.as_call_node().expect("kind matched");
        is_operator = is_operator_method(call.name().as_slice());
        is_assignment = is_setter_call(&call);
    }
    Anc { span: node.span(), is_send: kind == NodeKind::CallNode, is_assignment, is_operator }
}

/// The subscribed kinds: every keyword construct that owns an `end`, plus
/// `CheckAssignment`'s assignment and call nodes.
const KINDS: &[NodeKind] = &[
    NodeKind::ClassNode,
    NodeKind::ModuleNode,
    NodeKind::SingletonClassNode,
    NodeKind::IfNode,
    NodeKind::UnlessNode,
    NodeKind::WhileNode,
    NodeKind::UntilNode,
    NodeKind::CaseNode,
    NodeKind::CaseMatchNode,
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

/// The keyword constructs `on_class`/`on_module`/`on_if`/`on_while`/
/// `on_until`/`on_case`/`on_case_match` fire for.
fn construct_of(node: &Node<'_>) -> Option<Construct> {
    match node {
        Node::ClassNode { .. } => {
            let class = node.as_class_node().expect("kind matched");
            Some(Construct {
                span: node.span(),
                keyword: class.class_keyword_loc().span(),
                end: Some(class.end_keyword_loc().span()),
            })
        }
        Node::ModuleNode { .. } => {
            let module = node.as_module_node().expect("kind matched");
            Some(Construct {
                span: node.span(),
                keyword: module.module_keyword_loc().span(),
                end: Some(module.end_keyword_loc().span()),
            })
        }
        _ => conditional_of(node),
    }
}

/// `rubocop-ast`'s `Node#conditional?`, as a keyword/`end` pair.
///
/// A Prism `elsif` link is an `IfNode` that shares the outer `if`'s
/// `end_keyword_loc`; whitequark gives the `elsif` no `end` location at
/// all, so it is skipped here to keep one offense per `end`. A ternary has
/// no `if` keyword and no `end`.
fn conditional_of(node: &Node<'_>) -> Option<Construct> {
    match node {
        Node::IfNode { .. } => {
            let if_node = node.as_if_node().expect("kind matched");
            let keyword = if_node.if_keyword_loc()?;
            if keyword.as_slice() == b"elsif" {
                return None;
            }
            Some(Construct {
                span: node.span(),
                keyword: keyword.span(),
                end: if_node.end_keyword_loc().map(|l| l.span()),
            })
        }
        Node::UnlessNode { .. } => {
            let unless = node.as_unless_node().expect("kind matched");
            Some(Construct {
                span: node.span(),
                keyword: unless.keyword_loc().span(),
                end: unless.end_keyword_loc().map(|l| l.span()),
            })
        }
        Node::WhileNode { .. } => {
            let while_node = node.as_while_node().expect("kind matched");
            Some(Construct {
                span: node.span(),
                keyword: while_node.keyword_loc().span(),
                end: while_node.closing_loc().map(|l| l.span()),
            })
        }
        Node::UntilNode { .. } => {
            let until = node.as_until_node().expect("kind matched");
            Some(Construct {
                span: node.span(),
                keyword: until.keyword_loc().span(),
                end: until.closing_loc().map(|l| l.span()),
            })
        }
        Node::CaseNode { .. } => {
            let case = node.as_case_node().expect("kind matched");
            Some(Construct {
                span: node.span(),
                keyword: case.case_keyword_loc().span(),
                end: Some(case.end_keyword_loc().span()),
            })
        }
        Node::CaseMatchNode { .. } => {
            let case = node.as_case_match_node().expect("kind matched");
            Some(Construct {
                span: node.span(),
                keyword: case.case_keyword_loc().span(),
                end: Some(case.end_keyword_loc().span()),
            })
        }
        _ => None,
    }
}
