//! `Performance/RegexpMatch`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/regexp_match.rb`.

use std::collections::HashMap;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use `match?` instead of `Regexp#match`, `String#match`, `Symbol#match`, `Regexp#===`, or `=~` when `MatchData` is not used.
#[derive(Debug, Clone)]
pub struct RegexpMatch {
    /// `minimum_target_ruby_version 2.4`.
    enabled: bool,
}

impl Rule for RegexpMatch {
    const META: RuleMeta = RuleMeta {
        name: "Performance/RegexpMatch",
        department: Department::Performance,
        summary: "Use `match?` instead of `Regexp#match`, `String#match`, `Symbol#match`, `Regexp#===`, or `=~` when `MatchData` is not used.",
        explanation: "In Ruby 2.4, `String#match?`, `Regexp#match?`, and `Symbol#match?` have been added. The methods are faster than `match`, because they avoid creating a `MatchData` object or saving backref. So, when `MatchData` is not used, use `match?` instead of `match`.\n\n```ruby\n# bad\nif x =~ /re/\n  do_something\nend\n\n# good\nif x.match?(/re/)\n  do_something\nend\n\n# good\nif x =~ /re/\n  do_something(Regexp.last_match)\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ProgramNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { enabled: options.target_ruby_version() >= 2.4 })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.enabled {
            return;
        }
        let mut collector = Collector::default();
        collector.visit(node, ctx);

        for cond in &collector.conds {
            if collector.last_match_used(cond) {
                continue;
            }
            ctx.report_with_fix(
                &Self::META,
                cond.span,
                cond.message.clone(),
                Fix { applicability: Applicability::Safe, edits: cond.edits.clone() },
            );
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// `match_method?` / `match_with_int_arg_method?`.
    Method,
    /// `match_operator?`.
    Operator,
    /// `match_threequals?`.
    Threequals,
}

struct MatchRec {
    start: u32,
    /// The begin position `next_match_pos` compares (the `if` branch start
    /// when the match is the condition of a modifier `if`/`unless`).
    adjusted_start: u32,
    scope: Option<usize>,
}

struct RefRec {
    start: u32,
    scope: Option<usize>,
}

struct CondRec {
    span: Span,
    adjusted_start: u32,
    scope: Option<usize>,
    message: String,
    edits: Vec<Edit>,
}

#[derive(Default)]
struct Collector {
    /// Body span of each `def`/`class`/`module` scope root.
    bodies: Vec<Option<Span>>,
    stack: Vec<usize>,
    matches: Vec<MatchRec>,
    refs: Vec<RefRec>,
    conds: Vec<CondRec>,
    /// Predicate span of each modifier `if`/`unless` -> start of its branch.
    modifier: HashMap<(u32, u32), u32>,
}

impl Collector {
    fn scope(&self) -> Option<usize> {
        self.stack.last().copied()
    }

    fn in_body(&self, scope: Option<usize>, start: u32) -> bool {
        match scope {
            None => true,
            Some(id) => self.bodies[id].is_some_and(|body| start >= body.start && start < body.end),
        }
    }

    fn visit(&mut self, node: &Node<'_>, ctx: &Context<'_>) {
        let mut scoped = false;
        match node.kind() {
            NodeKind::DefNode => {
                let body = node.as_def_node().and_then(|n| n.body()).map(|b| b.span());
                self.enter_scope(body);
                scoped = true;
            }
            NodeKind::ClassNode => {
                let body = node.as_class_node().and_then(|n| n.body()).map(|b| b.span());
                self.enter_scope(body);
                scoped = true;
            }
            NodeKind::ModuleNode => {
                let body = node.as_module_node().and_then(|n| n.body()).map(|b| b.span());
                self.enter_scope(body);
                scoped = true;
            }
            NodeKind::IfNode => {
                if let Some(if_node) = node.as_if_node() {
                    let is_if =
                        if_node.if_keyword_loc().is_some_and(|loc| ctx.text(loc.span()) == b"if");
                    if is_if && if_node.end_keyword_loc().is_none() {
                        self.note_modifier(
                            &if_node.predicate(),
                            if_node.statements().map(|s| s.as_node().span().start),
                        );
                    }
                    self.check_condition(&if_node.predicate(), ctx);
                }
            }
            NodeKind::UnlessNode => {
                if let Some(unless) = node.as_unless_node() {
                    if unless.end_keyword_loc().is_none() {
                        self.note_modifier(
                            &unless.predicate(),
                            unless.statements().map(|s| s.as_node().span().start),
                        );
                    }
                    self.check_condition(&unless.predicate(), ctx);
                }
            }
            NodeKind::CaseNode => {
                if let Some(case) = node.as_case_node() {
                    if case.predicate().is_none() {
                        for when in &case.conditions() {
                            let Some(when) = when.as_when_node() else { continue };
                            for condition in &when.conditions() {
                                self.check_condition(&condition, ctx);
                            }
                        }
                    }
                }
            }
            NodeKind::CallNode => {
                if let Some(call) = node.as_call_node() {
                    self.record_call(&call, ctx);
                }
            }
            NodeKind::BackReferenceReadNode | NodeKind::NumberedReferenceReadNode => {
                self.refs.push(RefRec { start: node.span().start, scope: self.scope() });
            }
            NodeKind::GlobalVariableReadNode => {
                if let Some(gvar) = node.as_global_variable_read_node() {
                    if matches!(
                        gvar.name().as_slice(),
                        b"$~"
                            | b"$MATCH"
                            | b"$PREMATCH"
                            | b"$POSTMATCH"
                            | b"$LAST_PAREN_MATCH"
                            | b"$LAST_MATCH_INFO"
                    ) {
                        self.refs.push(RefRec { start: node.span().start, scope: self.scope() });
                    }
                }
            }
            _ => {}
        }

        if let Some(unless) = node.as_unless_node() {
            // whitequark's `(if cond else_branch body)` visits the `else` first.
            self.visit(&unless.predicate(), ctx);
            if let Some(else_clause) = unless.else_clause() {
                self.visit(&else_clause.as_node(), ctx);
            }
            if let Some(statements) = unless.statements() {
                self.visit(&statements.as_node(), ctx);
            }
        } else {
            for_each_child(node, |child| self.visit(child, ctx));
        }

        if scoped {
            self.stack.pop();
        }
    }

    fn enter_scope(&mut self, body: Option<Span>) {
        self.bodies.push(body);
        self.stack.push(self.bodies.len() - 1);
    }

    fn note_modifier(&mut self, predicate: &Node<'_>, branch_start: Option<u32>) {
        if let Some(start) = branch_start {
            let span = predicate.span();
            self.modifier.insert((span.start, span.end), start);
        }
    }

    fn adjusted_start(&self, call: &CallNode<'_>) -> u32 {
        let node = call.as_node();
        let span = node.span();
        let has_block = call.block().is_some_and(|b| b.as_block_node().is_some());
        if has_block {
            return span.start;
        }
        self.modifier.get(&(span.start, span.end)).copied().unwrap_or(span.start)
    }

    fn record_call(&mut self, call: &CallNode<'_>, ctx: &Context<'_>) {
        if classify(call, ctx).is_some() {
            self.matches.push(MatchRec {
                start: call.as_node().span().start,
                adjusted_start: self.adjusted_start(call),
                scope: self.scope(),
            });
        }
        if is_last_match_call(call) {
            self.refs.push(RefRec { start: call.as_node().span().start, scope: self.scope() });
        }
    }

    fn check_condition(&mut self, cond: &Node<'_>, ctx: &Context<'_>) {
        let Some(call) = cond.as_call_node() else { return };
        if call.block().is_some_and(|b| b.as_block_node().is_some()) {
            return;
        }
        let Some(kind) = classify(&call, ctx) else { return };
        let Some(selector) = call.message_loc() else { return };
        let selector = selector.span();

        let current = String::from_utf8_lossy(ctx.text(selector)).into_owned();
        let negation = if call.name().as_slice() == b"!~" { "!" } else { "" };
        let message =
            format!("Use `{negation}match?` instead of `{current}` when `MatchData` is not used.");

        let edits = match kind {
            Kind::Method => vec![Edit::replace(selector, b"match?".to_vec())],
            Kind::Operator | Kind::Threequals => correct_operator(&call, ctx),
        };
        self.conds.push(CondRec {
            span: call.as_node().span(),
            adjusted_start: self.adjusted_start(&call),
            scope: self.scope(),
            message,
            edits,
        });
    }

    fn last_match_used(&self, cond: &CondRec) -> bool {
        let scope = cond.scope;
        let pos = cond.adjusted_start;
        let next = self
            .matches
            .iter()
            .find(|m| m.scope == scope && self.in_body(scope, m.start) && m.adjusted_start > pos)
            .map(|m| m.start);
        self.refs.iter().any(|r| {
            r.scope == scope
                && self.in_body(scope, r.start)
                && r.start >= pos
                && next.is_none_or(|next| r.start <= next)
        })
    }
}

/// `argument_list`: the call's arguments, a `&block` one of them.
fn argument_list<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut args: Vec<Node<'pr>> =
        call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
    if let Some(block) = call.block().filter(|block| block.as_block_argument_node().is_some()) {
        args.push(block);
    }
    args
}

fn is_regexp(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::RegularExpressionNode | NodeKind::InterpolatedRegularExpressionNode
    )
}

fn is_str(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::StringNode | NodeKind::SourceFileNode)
}

fn is_sym(node: &Node<'_>) -> bool {
    node.kind() == NodeKind::SymbolNode
}

fn is_const(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
}

/// `TYPES_IMPLEMENTING_MATCH`: `const regexp str sym`.
fn implements_match(node: &Node<'_>) -> bool {
    is_const(node) || is_regexp(node) || is_str(node) || is_sym(node)
}

/// `match_node?` over a `send` (a call without safe navigation; a literal
/// block does not matter, the `send` is the block's child).
fn classify(call: &CallNode<'_>, ctx: &Context<'_>) -> Option<Kind> {
    if call.is_safe_navigation() {
        return None;
    }
    let args = argument_list(call);
    match call.name().as_slice() {
        b"match" => {
            let method = match args.as_slice() {
                [arg] => {
                    is_regexp(arg)
                        || is_str(arg)
                        || is_sym(arg)
                        || call
                            .receiver()
                            .is_some_and(|recv| is_regexp(&recv) || is_str(&recv) || is_sym(&recv))
                }
                _ => false,
            };
            let int_arg =
                matches!(args.as_slice(), [_, second] if second.as_integer_node().is_some());
            (method || int_arg).then_some(Kind::Method)
        }
        b"=~" | b"!~" => (call.receiver().is_some() && args.len() == 1).then_some(Kind::Operator),
        b"===" => {
            let regexp = call.receiver()?.as_regular_expression_node()?;
            if regexp.content_loc().span().is_empty() || args.len() != 1 {
                return None;
            }
            // `(regopt)` / `(regopt _)`: at most one distinct flag.
            let closing = ctx.text(regexp.closing_loc().span());
            let mut flags: Vec<u8> = closing.iter().skip(1).copied().collect();
            flags.sort_unstable();
            flags.dedup();
            (flags.len() <= 1).then_some(Kind::Threequals)
        }
        _ => None,
    }
}

fn is_last_match_call(call: &CallNode<'_>) -> bool {
    if call.is_safe_navigation() || call.name().as_slice() != b"last_match" {
        return false;
    }
    let Some(recv) = call.receiver() else { return false };
    let regexp_const = match recv.kind() {
        NodeKind::ConstantReadNode => {
            recv.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"Regexp")
        }
        NodeKind::ConstantPathNode => recv.as_constant_path_node().is_some_and(|c| {
            c.parent().is_none() && c.name().is_some_and(|n| n.as_slice() == b"Regexp")
        }),
        _ => false,
    };
    regexp_const && argument_list(call).len() <= 1
}

/// `correct_operator` for `=~`, `!~` and `===`.
fn correct_operator(call: &CallNode<'_>, ctx: &Context<'_>) -> Vec<Edit> {
    let Some(recv) = call.receiver() else { return Vec::new() };
    let args = argument_list(call);
    let Some(arg) = args.first() else { return Vec::new() };
    let negated = call.name().as_slice() == b"!~";
    let (recv_span, arg_span) = (recv.span(), arg.span());
    let op_range = Span::new(recv_span.end, arg_span.start);

    let (replacement, swap): (&[u8], bool) = if implements_match(&recv) {
        (b".match?(", false)
    } else if implements_match(arg) {
        (b".match?(", true)
    } else {
        (b"&.match?(", false)
    };
    let close = !ctx.text(op_range).ends_with(b"(");

    let mut recv_text = if swap { ctx.text(arg_span).to_vec() } else { Vec::new() };
    let mut edits = Vec::new();
    if negated {
        if swap {
            recv_text.insert(0, b'!');
        } else {
            edits.push(Edit::insert(recv_span.start, b"!".to_vec()));
        }
    }
    edits.push(Edit::replace(op_range, replacement.to_vec()));
    if swap {
        edits.push(Edit::replace(recv_span, recv_text));
        let mut arg_text = ctx.text(recv_span).to_vec();
        if close {
            arg_text.push(b')');
        }
        edits.push(Edit::replace(arg_span, arg_text));
    } else if close {
        edits.push(Edit::insert(arg_span.end, b")".to_vec()));
    }
    edits
}
