//! `Style/MethodCallWithArgsParentheses`, ported from RuboCop's
//! `lib/rubocop/cop/style/method_call_with_args_parentheses.rb` plus its
//! `RequireParentheses`/`OmitParentheses` mixins
//! (`lib/rubocop/cop/style/method_call_with_args_parentheses/*.rb`).
//!
//! Only `on_send`/`on_csend`/`on_yield` are hooked upstream; `super` is
//! never visited (dead `super_call_without_arguments?` check kept out of
//! this port). [`Target`] unifies `CallNode` (`send`/`csend`) and
//! `YieldNode`: `YieldNode` includes `MethodDispatchNode` upstream with
//! `method_name` hard-wired to `:yield` and `receiver` to `nil`, so the same
//! `require_parentheses`/`omit_parentheses` bodies apply to both.
//!
//! Prism has no parent pointers, so a `(kind, span) -> parent` map is built
//! once from `file_start` (the [`redundant_safe_navigation`] pattern),
//! alongside the list of every `Target` in the file. whitequark's `unless`
//! swap, `:begin`/`:kwbegin` elision, and `if`/`ternary` unification are
//! handled the same way `map_compact_with_conditional_block.rs` documents;
//! this cop does not need them beyond `conditional?`/`class_type?` kind
//! checks.
//!
//! The upstream `OmitParentheses#on_investigation_end` reparse-verification
//! safety net (confirms an omission would not change how the code parses,
//! by literally re-parsing a corrected copy) is not reproduced; the
//! structural predicates below are its intended first line of defence and
//! are what the fixtures exercise. See `blind_spots`.

use std::collections::HashMap;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::node::{BlockNode, CallNode, YieldNode};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

/// RuboCop's `RequireParentheses::REQUIRE_MSG`.
const REQUIRE_MSG: &str = "Use parentheses for method calls with arguments.";
/// RuboCop's `OmitParentheses::OMIT_MSG`.
const OMIT_MSG: &str = "Omit parentheses for method calls with arguments.";

/// `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    RequireParentheses,
    OmitParentheses,
}

/// Use parentheses for method calls with arguments.
#[derive(Debug, Clone)]
pub struct MethodCallWithArgsParentheses {
    style: Style,
    allowed_methods: Vec<String>,
    allowed_patterns: Vec<Regex>,
    ignore_macros: bool,
    included_macros: Vec<Vec<u8>>,
    included_macro_patterns: Vec<Regex>,
    /// The four `AllowParenthesesIn*` `omit_parentheses` flags, packed to keep this struct under
    /// clippy's bool-field limit.
    allow: AllowFlags,
}

/// `AllowParenthesesInMultilineCall`/`InChaining`/`InCamelCaseMethod`/`InStringInterpolation`.
#[derive(Debug, Clone, Copy, Default)]
struct AllowFlags(u8);

impl AllowFlags {
    const MULTILINE: u8 = 1 << 0;
    const CHAINING: u8 = 1 << 1;
    const CAMEL_CASE: u8 = 1 << 2;
    const STRING_INTERPOLATION: u8 = 1 << 3;

    fn set(mut self, flag: u8, value: bool) -> Self {
        if value {
            self.0 |= flag;
        }
        self
    }

    const fn multiline(self) -> bool {
        self.0 & Self::MULTILINE != 0
    }

    const fn chaining(self) -> bool {
        self.0 & Self::CHAINING != 0
    }

    const fn camel_case(self) -> bool {
        self.0 & Self::CAMEL_CASE != 0
    }

    const fn string_interpolation(self) -> bool {
        self.0 & Self::STRING_INTERPOLATION != 0
    }
}

impl Rule for MethodCallWithArgsParentheses {
    const META: RuleMeta = RuleMeta {
        name: "Style/MethodCallWithArgsParentheses",
        department: Department::Style,
        summary: "Use parentheses for method calls with arguments.",
        explanation: "\
In the default style (require_parentheses), macro methods are allowed. Additional methods can be \
added to the `AllowedMethods` or `AllowedPatterns` list. These options are valid only in the \
default style. Macros can be included by either setting `IgnoreMacros` to false, adding specific \
macros to the `IncludedMacros` list, or using `IncludedMacroPatterns` for pattern-based matching.

In the alternative style (omit_parentheses), `AllowParenthesesInChaining`,
`AllowParenthesesInMultilineCall`, and `AllowParenthesesInCamelCaseMethod` allow parentheses in \
those specific cases.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("require_parentheses"),
                allowed: &["require_parentheses", "omit_parentheses"],
                doc: "Whether method calls with arguments require or omit parentheses.",
            },
            ConfigOption {
                name: "IgnoreMacros",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "`require_parentheses` only: whether macro calls are exempt.",
            },
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "`require_parentheses` only: method names always allowed without parens.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "`require_parentheses` only: method name regex patterns always allowed.",
            },
            ConfigOption {
                name: "IncludedMacros",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "`require_parentheses` only: macro method names never exempted.",
            },
            ConfigOption {
                name: "IncludedMacroPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "`require_parentheses` only: macro method name regex patterns never \
exempted.",
            },
            ConfigOption {
                name: "AllowParenthesesInMultilineCall",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "`omit_parentheses` only: whether a multi-line call may keep parentheses.",
            },
            ConfigOption {
                name: "AllowParenthesesInChaining",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "`omit_parentheses` only: whether the last call in a chain may keep \
parentheses.",
            },
            ConfigOption {
                name: "AllowParenthesesInCamelCaseMethod",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "`omit_parentheses` only: whether a capitalized method call may keep \
parentheses.",
            },
            ConfigOption {
                name: "AllowParenthesesInStringInterpolation",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "`omit_parentheses` only: whether a call inside string interpolation may \
keep parentheses.",
            },
        ],
        blind_spots: "\
The upstream `OmitParentheses` reparse-verification safety net (`on_investigation_end`'s
`verified_by_reparse`, which re-parses a corrected copy to confirm the omission does not change
how the code parses) is not reproduced; this port relies solely on the structural predicates
(`legitimate_call_with_parentheses?` and friends) the fixtures exercise. `class_constructor?`
(`Class.new`/`Module.new`/`Struct.new`/`Data.define`, used by `in_macro_scope?`'s parent check) and
the `any_block`/`if`-excluding-condition wrapper steps of `in_macro_scope?` are implemented but not
exercised by any fixture.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "omit_parentheses" => Style::OmitParentheses,
            _ => Style::RequireParentheses,
        };
        Ok(Self {
            style,
            allowed_methods: options.str_list("AllowedMethods"),
            allowed_patterns: compile_patterns(&options.str_list("AllowedPatterns")),
            ignore_macros: options.bool("IgnoreMacros"),
            included_macros: options
                .str_list("IncludedMacros")
                .into_iter()
                .map(String::into_bytes)
                .collect(),
            included_macro_patterns: compile_patterns(&options.str_list("IncludedMacroPatterns")),
            allow: AllowFlags::default()
                .set(AllowFlags::MULTILINE, options.bool("AllowParenthesesInMultilineCall"))
                .set(AllowFlags::CHAINING, options.bool("AllowParenthesesInChaining"))
                .set(AllowFlags::CAMEL_CASE, options.bool("AllowParenthesesInCamelCaseMethod"))
                .set(
                    AllowFlags::STRING_INTERPOLATION,
                    options.bool("AllowParenthesesInStringInterpolation"),
                ),
        })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let analysis = Analysis::build(root);

        let mut offenses: Vec<(Span, &'static str, Fix)> = Vec::new();
        for target in &analysis.targets {
            let found = match self.style {
                Style::RequireParentheses => self.require_parentheses(target, &analysis, ctx),
                Style::OmitParentheses => self.omit_parentheses(target, &analysis, ctx),
            };
            if let Some(offense) = found {
                offenses.push(offense);
            }
        }

        for (span, message, fix) in offenses {
            ctx.report_with_fix(&Self::META, span, message, fix);
        }
    }
}

fn compile_patterns(patterns: &[String]) -> Vec<Regex> {
    patterns.iter().filter_map(|p| Regex::new(p).ok()).collect()
}

/// Unifies `CallNode` (`send`/`csend`) and `YieldNode`: RuboCop's shared
/// `on_send`/`on_csend`/`on_yield` dispatch to the same `require_parentheses`/
/// `omit_parentheses` bodies, since `YieldNode` includes `MethodDispatchNode`
/// with `method_name` hard-wired to `:yield` and `receiver` to `nil`.
#[derive(Debug, Clone, Copy)]
enum Target<'pr> {
    Call(CallNode<'pr>),
    Yield(YieldNode<'pr>),
}

impl<'pr> Target<'pr> {
    fn as_node(&self) -> Node<'pr> {
        match self {
            Self::Call(c) => c.as_node(),
            Self::Yield(y) => y.as_node(),
        }
    }

    /// RuboCop's `node.yield_type? ? loc.keyword : loc.selector`.
    fn keyword_span(&self) -> Span {
        match self {
            Self::Call(c) => c.message_loc().map_or_else(|| c.as_node().span(), |l| l.span()),
            Self::Yield(y) => y.keyword_loc().span(),
        }
    }

    fn opening(&self) -> Option<Span> {
        match self {
            Self::Call(c) => c.opening_loc().map(|l| l.span()),
            Self::Yield(y) => y.lparen_loc().map(|l| l.span()),
        }
    }

    fn closing(&self) -> Option<Span> {
        match self {
            Self::Call(c) => c.closing_loc().map(|l| l.span()),
            Self::Yield(y) => y.rparen_loc().map(|l| l.span()),
        }
    }

    /// Whitequark's argument list: Prism keeps `&blk` in `block`, whitequark makes the
    /// `block_pass` the last argument.
    fn arguments(&self) -> Vec<Node<'pr>> {
        let (args, block_pass) = match self {
            Self::Call(c) => {
                (c.arguments(), c.block().filter(|b| b.kind() == NodeKind::BlockArgumentNode))
            }
            Self::Yield(y) => (y.arguments(), None),
        };
        let mut out: Vec<Node<'pr>> =
            args.map(|a| a.arguments().iter().collect()).unwrap_or_default();
        out.extend(block_pass);
        out
    }

    fn method_name(&self) -> &'pr [u8] {
        match self {
            Self::Call(c) => c.name().as_slice(),
            Self::Yield(_) => b"yield",
        }
    }

    fn receiver(&self) -> Option<Node<'pr>> {
        match self {
            Self::Call(c) => c.receiver(),
            Self::Yield(_) => None,
        }
    }

    fn block(&self) -> Option<BlockNode<'pr>> {
        match self {
            Self::Call(c) => c.block().and_then(|b| b.as_block_node()),
            Self::Yield(_) => None,
        }
    }

    /// RuboCop-AST's `MethodDispatchNode#setter_method?` (`loc?(:operator)`).
    fn equal_loc(&self) -> Option<Span> {
        match self {
            Self::Call(c) => c.equal_loc().map(|l| l.span()),
            Self::Yield(_) => None,
        }
    }
}

/// `(kind, span) -> parent` map plus every `Target` in the file, in source
/// order.
struct Analysis<'pr> {
    parent: HashMap<(NodeKind, Span), Node<'pr>>,
    targets: Vec<Target<'pr>>,
}

impl<'pr> Analysis<'pr> {
    fn build(root: Node<'pr>) -> Self {
        let mut analysis = Self { parent: HashMap::new(), targets: Vec::new() };
        analysis.visit(root);
        analysis
    }

    fn visit(&mut self, node: Node<'pr>) {
        if let Some(call) = node.as_call_node() {
            self.targets.push(Target::Call(call));
        } else if let Some(y) = node.as_yield_node() {
            self.targets.push(Target::Yield(y));
        }
        let mut children = Vec::new();
        for_each_child(&node, |child| children.push(*child));
        for child in children {
            self.parent.insert((child.kind(), child.span()), node);
            self.visit(child);
        }
    }

    fn parent_of(&self, node: &Node<'pr>) -> Option<Node<'pr>> {
        self.parent.get(&(node.kind(), node.span())).copied()
    }

    /// The nearest ancestor whitequark would actually expose as `#parent`: skips an
    /// `ArgumentsNode` layer (Prism's own child field with no whitequark counterpart -- a `send`
    /// node's arguments are direct children there) and a single-statement `StatementsNode`
    /// (whitequark elides the `:begin` wrapper for exactly one statement).
    fn logical_parent_of(&self, node: &Node<'pr>) -> Option<Node<'pr>> {
        let mut current = self.parent_of(node)?;
        loop {
            if current.kind() == NodeKind::ArgumentsNode {
                current = self.parent_of(&current)?;
                continue;
            }
            if let Some(stmts) = current.as_statements_node() {
                if stmts.body().len() == 1 {
                    current = self.parent_of(&current)?;
                    continue;
                }
            }
            if current.kind() == NodeKind::ProgramNode {
                return None;
            }
            return Some(current);
        }
    }
}

// ---------------------------------------------------------------------
// Shared: `args_begin`/`args_end`/`args_parenthesized?`.
// ---------------------------------------------------------------------

/// RuboCop's `args_parenthesized?`.
fn args_parenthesized(target: &Target<'_>) -> bool {
    match target.arguments().as_slice() {
        [only] => only.kind() == NodeKind::ParenthesesNode,
        _ => false,
    }
}

/// RuboCop's `args_begin`.
fn args_begin_span(target: &Target<'_>) -> Span {
    let end = target.keyword_span().end;
    let resize_by = if args_parenthesized(target) { 2 } else { 1 };
    Span::new(end, end + resize_by)
}

/// RuboCop's `args_end`.
fn args_end(target: &Target<'_>) -> u32 {
    target.as_node().span().end
}

// ---------------------------------------------------------------------
// `RequireParentheses`.
// ---------------------------------------------------------------------

impl MethodCallWithArgsParentheses {
    fn require_parentheses(
        &self,
        target: &Target<'_>,
        analysis: &Analysis<'_>,
        _ctx: &Context<'_>,
    ) -> Option<(Span, &'static str, Fix)> {
        let name = target.method_name();
        if allowed_method_name(name, &self.allowed_methods, &self.allowed_patterns) {
            return None;
        }
        if self.eligible_for_parentheses_omission(target, analysis) {
            return None;
        }
        if target.arguments().is_empty() || target.opening().is_some() {
            return None;
        }

        let span = target.as_node().span();
        let mut edits = vec![Edit::replace(args_begin_span(target), b"(".to_vec())];
        if !args_parenthesized(target) {
            edits.push(Edit::insert(args_end(target), b")".to_vec()));
        }
        Some((span, REQUIRE_MSG, Fix { applicability: Applicability::Safe, edits }))
    }

    /// RuboCop's `eligible_for_parentheses_omission?`.
    fn eligible_for_parentheses_omission(
        &self,
        target: &Target<'_>,
        analysis: &Analysis<'_>,
    ) -> bool {
        let name = target.method_name();
        is_operator_method(name)
            || target.equal_loc().is_some()
            || self.ignored_macro(target, analysis)
    }

    /// RuboCop's `ignored_macro?`.
    fn ignored_macro(&self, target: &Target<'_>, analysis: &Analysis<'_>) -> bool {
        if !self.ignore_macros {
            return false;
        }
        if !is_macro(target, analysis) {
            return false;
        }
        let name = target.method_name();
        if self.included_macros.iter().any(|m| m.as_slice() == name) {
            return false;
        }
        if matches_any(&self.included_macro_patterns, name) {
            return false;
        }
        true
    }
}

/// RuboCop's `allowed_method?`/`matches_allowed_pattern?` combined.
fn allowed_method_name(
    name: &[u8],
    allowed_methods: &[String],
    allowed_patterns: &[Regex],
) -> bool {
    allowed_methods.iter().any(|m| m.as_bytes() == name) || matches_any(allowed_patterns, name)
}

fn matches_any(patterns: &[Regex], name: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(name) else { return false };
    patterns.iter().any(|p| p.is_match(text))
}

/// RuboCop-AST's `MethodDispatchNode#macro?`: `!receiver && in_macro_scope?`.
fn is_macro(target: &Target<'_>, analysis: &Analysis<'_>) -> bool {
    target.receiver().is_none() && in_macro_scope(target.as_node(), analysis)
}

/// RuboCop-AST's `in_macro_scope?`, mapped onto Prism's tree shape: `ProgramNode` is `root?`; a
/// `BlockNode`/`LambdaNode` hangs under its call, so the whitequark `any_block` wrapper continues
/// from that call (or matches `class_constructor?` when the call is one); a `BeginNode` with
/// `rescue`/`ensure` is whitequark's `kwbegin(rescue|ensure ...)`, whose body is not a wrapper.
fn in_macro_scope(node: Node<'_>, analysis: &Analysis<'_>) -> bool {
    let Some(parent) = analysis.parent_of(&node) else { return true };
    match parent.kind() {
        NodeKind::ProgramNode
        | NodeKind::SingletonClassNode
        | NodeKind::ClassNode
        | NodeKind::ModuleNode => true,
        _ if is_class_constructor(&parent) => true,
        NodeKind::BlockNode => match analysis.parent_of(&parent) {
            Some(call) if is_class_constructor(&call) => true,
            Some(call) => in_macro_scope(call, analysis),
            None => true,
        },
        NodeKind::BeginNode => {
            let begin = parent.as_begin_node();
            let guarded =
                begin.is_some_and(|b| b.rescue_clause().is_some() || b.ensure_clause().is_some());
            !guarded && in_macro_scope(parent, analysis)
        }
        NodeKind::StatementsNode
        | NodeKind::ParenthesesNode
        | NodeKind::LambdaNode
        | NodeKind::ElseNode => in_macro_scope(parent, analysis),
        NodeKind::IfNode | NodeKind::UnlessNode => {
            // Excludes the condition: only the `then`/`else` branches count as a wrapper.
            !is_condition_of(&parent, &node) && in_macro_scope(parent, analysis)
        }
        _ => false,
    }
}

fn is_condition_of(cond_owner: &Node<'_>, node: &Node<'_>) -> bool {
    let predicate = if let Some(if_node) = cond_owner.as_if_node() {
        Some(if_node.predicate())
    } else {
        cond_owner.as_unless_node().map(|u| u.predicate())
    };
    predicate.is_some_and(|p| p.kind() == node.kind() && p.span() == node.span())
}

/// RuboCop-AST's `Node#class_constructor?`.
fn is_class_constructor(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|call| is_constructor_call(&call))
}

fn is_constructor_call(call: &CallNode<'_>) -> bool {
    let Some(receiver) = call.receiver() else { return false };
    let Some(name) = ruby_ast::ext::const_name(&receiver) else { return false };
    match call.name().as_slice() {
        b"new" => matches!(name.as_str(), "Class" | "Module" | "Struct"),
        b"define" => name == "Data",
        _ => false,
    }
}

/// RuboCop-AST's `OPERATOR_METHODS`.
fn is_operator_method(name: &[u8]) -> bool {
    matches!(
        name,
        b"|" | b"^"
            | b"&"
            | b"<=>"
            | b"=="
            | b"==="
            | b"=~"
            | b">"
            | b">="
            | b"<"
            | b"<="
            | b"<<"
            | b">>"
            | b"+"
            | b"-"
            | b"*"
            | b"/"
            | b"%"
            | b"**"
            | b"~"
            | b"+@"
            | b"-@"
            | b"!@"
            | b"~@"
            | b"[]"
            | b"[]="
            | b"!"
            | b"!="
            | b"!~"
            | b"`"
    )
}

// ---------------------------------------------------------------------
// `OmitParentheses`.
// ---------------------------------------------------------------------

impl MethodCallWithArgsParentheses {
    fn omit_parentheses(
        &self,
        target: &Target<'_>,
        analysis: &Analysis<'_>,
        ctx: &Context<'_>,
    ) -> Option<(Span, &'static str, Fix)> {
        target.opening()?;
        if inside_endless_method_def(target, analysis) {
            return None;
        }
        if Self::require_parentheses_for_hash_value_omission(target, analysis, ctx) {
            return None;
        }
        if syntax_like_method_call(target) {
            return None;
        }
        if method_call_before_constant_resolution(target, analysis) {
            return None;
        }
        if self.legitimate_call_with_parentheses(target, analysis, ctx) {
            return None;
        }
        if self.allowed_camel_case_method_call(target) {
            return None;
        }
        if self.allow.string_interpolation() && inside_string_interpolation(target, analysis) {
            return None;
        }

        let open = target.opening()?;
        let close = target.closing()?;
        let span = Span::new(open.start, close.end);

        let mut edits = Vec::new();
        let range = args_begin_span(target);
        if parens_at_end_of_multiline_call(target, ctx) {
            let expanded = ctx.with_surrounding_space(range, Side::Right, false, false);
            edits.push(Edit::replace(expanded, b" \\".to_vec()));
        } else {
            edits.push(Edit::replace(range, b" ".to_vec()));
        }
        edits.push(Edit::delete(close));

        Some((span, OMIT_MSG, Fix { applicability: Applicability::Unsafe, edits }))
    }

    /// RuboCop's `require_parentheses_for_hash_value_omission?`.
    fn require_parentheses_for_hash_value_omission(
        target: &Target<'_>,
        analysis: &Analysis<'_>,
        ctx: &Context<'_>,
    ) -> bool {
        let Some(last) = target.arguments().last().copied() else { return false };
        let Some(pairs) = hash_pairs(&last) else { return false };
        let Some(last_pair) = pairs.last() else { return false };
        if !ctx.text(last_pair.span()).ends_with(b":") {
            return false;
        }
        let Some(parent) = analysis.logical_parent_of(&target.as_node()) else { return false };
        if is_conditional(parent.kind()) || parent.kind() == NodeKind::WhenNode {
            return true;
        }
        if ctx.is_single_line(parent.span()) {
            return true;
        }
        !last_expression(target, analysis)
    }

    /// RuboCop's `legitimate_call_with_parentheses?`.
    fn legitimate_call_with_parentheses(
        &self,
        target: &Target<'_>,
        analysis: &Analysis<'_>,
        ctx: &Context<'_>,
    ) -> bool {
        call_in_literals(target, analysis)
            || analysis
                .logical_parent_of(&target.as_node())
                .is_some_and(|p| p.kind() == NodeKind::WhenNode)
            || call_with_ambiguous_arguments(target, analysis, ctx)
            || call_in_logical_operators(target, analysis, ctx)
            || call_in_optional_arguments(target, analysis)
            || call_in_single_line_inheritance(target, analysis, ctx)
            || (self.allow.multiline() && !ctx.is_single_line(target.as_node().span()))
            || (self.allow.chaining() && allowed_chained_call_with_parentheses(target))
            || assignment_in_condition(target, analysis)
            || forwards_anonymous_rest_arguments(target)
            || call_in_match_pattern(target, analysis)
    }

    /// RuboCop's `allowed_camel_case_method_call?`.
    fn allowed_camel_case_method_call(&self, target: &Target<'_>) -> bool {
        is_camel_case(target.method_name())
            && (target.arguments().is_empty() || self.allow.camel_case())
    }
}

/// RuboCop's `inside_endless_method_def?`.
fn inside_endless_method_def(target: &Target<'_>, analysis: &Analysis<'_>) -> bool {
    if target.arguments().is_empty() {
        return false;
    }
    let mut cur = target.as_node();
    while let Some(parent) = analysis.parent_of(&cur) {
        if let Some(def) = parent.as_def_node() {
            if def.equal_loc().is_some() {
                return true;
            }
        }
        cur = parent;
    }
    false
}

fn hash_pairs<'pr>(node: &Node<'pr>) -> Option<Vec<Node<'pr>>> {
    if let Some(h) = node.as_hash_node() {
        return Some(h.elements().iter().collect());
    }
    if let Some(h) = node.as_keyword_hash_node() {
        return Some(h.elements().iter().collect());
    }
    None
}

fn is_conditional(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::IfNode
            | NodeKind::UnlessNode
            | NodeKind::WhileNode
            | NodeKind::UntilNode
            | NodeKind::CaseNode
            | NodeKind::CaseMatchNode
    )
}

/// RuboCop's `last_expression?`.
fn last_expression(target: &Target<'_>, analysis: &Analysis<'_>) -> bool {
    let node = target.as_node();
    let check_node = match analysis.parent_of(&node) {
        Some(parent) if is_assignment_like(&parent) => parent,
        _ => node,
    };
    right_sibling(&check_node, analysis).is_none()
}

/// RuboCop's generic `right_sibling` within a `StatementsNode`.
fn right_sibling<'pr>(node: &Node<'pr>, analysis: &Analysis<'pr>) -> Option<Node<'pr>> {
    let parent = analysis.parent_of(node)?;
    let stmts = parent.as_statements_node()?;
    let items: Vec<Node<'pr>> = stmts.body().iter().collect();
    let idx = items.iter().position(|n| n.kind() == node.kind() && n.span() == node.span())?;
    items.get(idx + 1).copied()
}

/// RuboCop-AST's `Node#assignment?`, with `MethodDispatchNode#assignment?`'s
/// `setter_method?` override for a `send`/`csend` parent.
fn is_assignment_like(node: &Node<'_>) -> bool {
    if let Some(call) = node.as_call_node() {
        return call.equal_loc().is_some();
    }
    matches!(
        node.kind(),
        NodeKind::LocalVariableWriteNode
            | NodeKind::LocalVariableAndWriteNode
            | NodeKind::LocalVariableOrWriteNode
            | NodeKind::LocalVariableOperatorWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::InstanceVariableAndWriteNode
            | NodeKind::InstanceVariableOrWriteNode
            | NodeKind::InstanceVariableOperatorWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::ClassVariableAndWriteNode
            | NodeKind::ClassVariableOrWriteNode
            | NodeKind::ClassVariableOperatorWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::GlobalVariableOperatorWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantAndWriteNode
            | NodeKind::ConstantOrWriteNode
            | NodeKind::ConstantOperatorWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::ConstantPathAndWriteNode
            | NodeKind::ConstantPathOrWriteNode
            | NodeKind::ConstantPathOperatorWriteNode
            | NodeKind::MultiWriteNode
            | NodeKind::IndexAndWriteNode
            | NodeKind::IndexOrWriteNode
            | NodeKind::IndexOperatorWriteNode
            | NodeKind::CallAndWriteNode
            | NodeKind::CallOrWriteNode
            | NodeKind::CallOperatorWriteNode
    )
}

/// The operator's own span start, for the write-node kinds
/// [`is_assignment_like`] recognises (`MethodDispatchNode`'s `setter_method?`
/// covered via `equal_loc`; every other write node via `operator_loc`).
fn assignment_operator_start(node: &Node<'_>) -> Option<u32> {
    if let Some(call) = node.as_call_node() {
        return call.equal_loc().map(|l| l.span().start);
    }
    macro_rules! op {
        ($accessor:ident) => {
            node.$accessor().map(|n| n.operator_loc().span().start)
        };
    }
    macro_rules! binop {
        ($accessor:ident) => {
            node.$accessor().map(|n| n.binary_operator_loc().span().start)
        };
    }
    match node.kind() {
        NodeKind::LocalVariableWriteNode => op!(as_local_variable_write_node),
        NodeKind::LocalVariableAndWriteNode => op!(as_local_variable_and_write_node),
        NodeKind::LocalVariableOrWriteNode => op!(as_local_variable_or_write_node),
        NodeKind::LocalVariableOperatorWriteNode => binop!(as_local_variable_operator_write_node),
        NodeKind::InstanceVariableWriteNode => op!(as_instance_variable_write_node),
        NodeKind::InstanceVariableAndWriteNode => op!(as_instance_variable_and_write_node),
        NodeKind::InstanceVariableOrWriteNode => op!(as_instance_variable_or_write_node),
        NodeKind::InstanceVariableOperatorWriteNode => {
            binop!(as_instance_variable_operator_write_node)
        }
        NodeKind::ClassVariableWriteNode => op!(as_class_variable_write_node),
        NodeKind::ClassVariableAndWriteNode => op!(as_class_variable_and_write_node),
        NodeKind::ClassVariableOrWriteNode => op!(as_class_variable_or_write_node),
        NodeKind::ClassVariableOperatorWriteNode => binop!(as_class_variable_operator_write_node),
        NodeKind::GlobalVariableWriteNode => op!(as_global_variable_write_node),
        NodeKind::GlobalVariableAndWriteNode => op!(as_global_variable_and_write_node),
        NodeKind::GlobalVariableOrWriteNode => op!(as_global_variable_or_write_node),
        NodeKind::GlobalVariableOperatorWriteNode => binop!(as_global_variable_operator_write_node),
        NodeKind::ConstantWriteNode => op!(as_constant_write_node),
        NodeKind::ConstantAndWriteNode => op!(as_constant_and_write_node),
        NodeKind::ConstantOrWriteNode => op!(as_constant_or_write_node),
        NodeKind::ConstantOperatorWriteNode => binop!(as_constant_operator_write_node),
        NodeKind::ConstantPathWriteNode => op!(as_constant_path_write_node),
        NodeKind::ConstantPathAndWriteNode => op!(as_constant_path_and_write_node),
        NodeKind::ConstantPathOrWriteNode => op!(as_constant_path_or_write_node),
        NodeKind::ConstantPathOperatorWriteNode => binop!(as_constant_path_operator_write_node),
        NodeKind::MultiWriteNode => op!(as_multi_write_node),
        NodeKind::IndexAndWriteNode => op!(as_index_and_write_node),
        NodeKind::IndexOrWriteNode => op!(as_index_or_write_node),
        NodeKind::IndexOperatorWriteNode => binop!(as_index_operator_write_node),
        NodeKind::CallAndWriteNode => op!(as_call_and_write_node),
        NodeKind::CallOrWriteNode => op!(as_call_or_write_node),
        NodeKind::CallOperatorWriteNode => binop!(as_call_operator_write_node),
        _ => None,
    }
}

/// RuboCop's `assigned_before?`.
fn assigned_before(assign_node: &Node<'_>, target_span_start: u32) -> bool {
    is_assignment_like(assign_node)
        && assignment_operator_start(assign_node).is_some_and(|op| op < target_span_start)
}

/// RuboCop's `call_in_literals?`.
fn call_in_literals(target: &Target<'_>, analysis: &Analysis<'_>) -> bool {
    let Some(parent) = analysis.logical_parent_of(&target.as_node()) else { return false };
    matches!(parent.kind(), NodeKind::AssocNode | NodeKind::ArrayNode | NodeKind::RangeNode)
        || is_splat(&parent)
        || is_ternary_if(&parent)
}

fn is_splat(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::SplatNode | NodeKind::AssocSplatNode | NodeKind::BlockArgumentNode
    )
}

fn is_ternary_if(node: &Node<'_>) -> bool {
    node.as_if_node().is_some_and(|i| i.if_keyword_loc().is_none())
}

/// RuboCop's `logical_operator?` helper (`operator_keyword? && logical_operator?`, which for
/// `AndNode`/`OrNode` reduces to: the operator's own text is `&&`/`||`, not the keyword spelling).
fn is_logical_operator(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let operator_loc = if let Some(and) = node.as_and_node() {
        and.operator_loc()
    } else if let Some(or) = node.as_or_node() {
        or.operator_loc()
    } else {
        return false;
    };
    matches!(ctx.text(operator_loc.span()), b"&&" | b"||")
}

/// RuboCop's `call_in_logical_operators?`.
fn call_in_logical_operators(
    target: &Target<'_>,
    analysis: &Analysis<'_>,
    ctx: &Context<'_>,
) -> bool {
    let Some(parent) = analysis.logical_parent_of(&target.as_node()) else { return false };
    if is_logical_operator(&parent, ctx) {
        return true;
    }
    parent.as_call_node().is_some_and(|c| {
        c.arguments()
            .is_some_and(|a| a.arguments().iter().any(|arg| is_logical_operator(&arg, ctx)))
    })
}

/// RuboCop's `call_in_optional_arguments?`.
fn call_in_optional_arguments(target: &Target<'_>, analysis: &Analysis<'_>) -> bool {
    analysis.logical_parent_of(&target.as_node()).is_some_and(|p| {
        matches!(p.kind(), NodeKind::OptionalParameterNode | NodeKind::OptionalKeywordParameterNode)
    })
}

/// RuboCop's `call_in_single_line_inheritance?`.
fn call_in_single_line_inheritance(
    target: &Target<'_>,
    analysis: &Analysis<'_>,
    ctx: &Context<'_>,
) -> bool {
    analysis
        .logical_parent_of(&target.as_node())
        .is_some_and(|p| p.kind() == NodeKind::ClassNode && ctx.is_single_line(p.span()))
}

/// RuboCop's `allowed_chained_call_with_parentheses?` (config check already applied by the
/// caller).
fn allowed_chained_call_with_parentheses(target: &Target<'_>) -> bool {
    let Some(previous) = target.receiver() else { return false };
    let Some(call) = previous.as_call_node() else { return false };
    call.opening_loc().is_some() || allowed_chained_call_with_parentheses(&Target::Call(call))
}

/// RuboCop's `assignment_in_condition?`.
fn assignment_in_condition(target: &Target<'_>, analysis: &Analysis<'_>) -> bool {
    let Some(parent) = analysis.logical_parent_of(&target.as_node()) else { return false };
    let Some(grandparent) = analysis.logical_parent_of(&parent) else { return false };
    is_assignment_like(&parent)
        && (is_conditional(grandparent.kind()) || grandparent.kind() == NodeKind::WhenNode)
}

/// RuboCop's `forwards_anonymous_rest_arguments?`.
fn forwards_anonymous_rest_arguments(target: &Target<'_>) -> bool {
    let Some(last) = target.arguments().last().copied() else { return false };
    if last.as_splat_node().is_some_and(|s| s.expression().is_none()) {
        return true;
    }
    hash_pairs(&last).is_some_and(|pairs| {
        pairs.iter().any(|p| p.as_assoc_splat_node().is_some_and(|a| a.value().is_none()))
    })
}

/// RuboCop's `call_in_match_pattern?`.
fn call_in_match_pattern(target: &Target<'_>, analysis: &Analysis<'_>) -> bool {
    analysis.logical_parent_of(&target.as_node()).is_some_and(|p| {
        matches!(p.kind(), NodeKind::MatchPredicateNode | NodeKind::MatchRequiredNode)
    })
}

/// RuboCop's `call_with_ambiguous_arguments?`.
fn call_with_ambiguous_arguments(
    target: &Target<'_>,
    analysis: &Analysis<'_>,
    ctx: &Context<'_>,
) -> bool {
    if call_with_braced_block(target) {
        return true;
    }
    if call_in_argument_with_block(target, analysis) {
        return true;
    }
    if call_as_argument_or_chain(target, analysis) {
        return true;
    }
    if call_in_match_pattern(target, analysis) {
        return true;
    }
    if hash_literal_in_arguments(target) {
        return true;
    }
    if ambiguous_range_argument(target) {
        return true;
    }
    let mut found = false;
    let mut scan = |root: Node<'_>| {
        if found {
            return;
        }
        if matches!(root.kind(), NodeKind::ForwardingArgumentsNode | NodeKind::BlockNode)
            || is_ambiguous_literal(root, analysis, ctx)
            || matches!(root.kind(), NodeKind::AndNode | NodeKind::OrNode)
        {
            found = true;
            return;
        }
        ruby_ast::each_descendant(&root, &mut |n| {
            if found {
                return;
            }
            if matches!(n.kind(), NodeKind::ForwardingArgumentsNode | NodeKind::BlockNode)
                || is_ambiguous_literal(*n, analysis, ctx)
                || matches!(n.kind(), NodeKind::AndNode | NodeKind::OrNode)
            {
                found = true;
            }
        });
    };
    if let Some(receiver) = target.receiver() {
        scan(receiver);
    }
    if let Target::Call(call) = target {
        if let Some(block) = call.block() {
            if block.kind() == NodeKind::BlockArgumentNode {
                scan(block);
            }
        }
    }
    for arg in target.arguments() {
        scan(arg);
    }
    found
}

fn call_with_braced_block(target: &Target<'_>) -> bool {
    target.block().is_some_and(|b| {
        b.opening_loc().span().start < b.opening_loc().span().end && is_brace_block(&b)
    })
}

fn is_brace_block(block: &BlockNode<'_>) -> bool {
    // Opening delimiter text is read by the caller via `ctx`; braces are the only non-`do`
    // spelling, so a one-byte opener identifies them without needing source text here.
    block.opening_loc().span().end - block.opening_loc().span().start == 1
}

/// RuboCop's `call_in_argument_with_block?`.
fn call_in_argument_with_block(target: &Target<'_>, analysis: &Analysis<'_>) -> bool {
    if target.block().is_none() {
        return false;
    }
    analysis.logical_parent_of(&target.as_node()).is_some_and(|p| is_call_like(p.kind()))
}

/// RuboCop's `call_as_argument_or_chain?`.
fn call_as_argument_or_chain(target: &Target<'_>, analysis: &Analysis<'_>) -> bool {
    let node = target.as_node();
    let Some(parent) = analysis.logical_parent_of(&node) else { return false };
    is_call_like(parent.kind()) && !assigned_before(&parent, node.span().start)
}

/// Every call-shaped kind: `send`/`csend`/`yield` plus the shorthand-assignment forms on a call
/// or index target (`CallAndWriteNode`, `IndexOperatorWriteNode`, ...), all of which whitequark
/// represents as the same `:send`/`:csend` type.
fn is_call_like(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::CallNode
            | NodeKind::YieldNode
            | NodeKind::SuperNode
            | NodeKind::CallAndWriteNode
            | NodeKind::CallOrWriteNode
            | NodeKind::CallOperatorWriteNode
            | NodeKind::IndexAndWriteNode
            | NodeKind::IndexOrWriteNode
            | NodeKind::IndexOperatorWriteNode
    )
}

/// RuboCop's `hash_literal_in_arguments?`.
fn hash_literal_in_arguments(target: &Target<'_>) -> bool {
    target.arguments().iter().any(|n| {
        is_braced_hash(n)
            || (n.as_call_node().is_some() && {
                let mut found = false;
                ruby_ast::each_descendant(&target.as_node(), &mut |d| {
                    if !found && is_braced_hash(d) {
                        found = true;
                    }
                });
                found
            })
    })
}

fn is_braced_hash(node: &Node<'_>) -> bool {
    node.as_hash_node().is_some()
}

/// RuboCop's `ambiguous_range_argument?`.
fn ambiguous_range_argument(target: &Target<'_>) -> bool {
    let args = target.arguments();
    if let Some(first) = args.first() {
        if first.as_range_node().is_some_and(|r| r.left().is_none()) {
            return true;
        }
    }
    if let Some(last) = args.last() {
        if last.as_range_node().is_some_and(|r| r.right().is_none()) {
            return true;
        }
    }
    false
}

fn is_ambiguous_literal(n: Node<'_>, analysis: &Analysis<'_>, ctx: &Context<'_>) -> bool {
    is_splat(&n) || is_ternary_if(&n) || is_slash_regexp(&n) || is_unary_literal(&n, analysis, ctx)
}

fn is_slash_regexp(n: &Node<'_>) -> bool {
    n.as_regular_expression_node()
        .is_some_and(|r| r.opening_loc().span().end - r.opening_loc().span().start == 1)
}

fn is_unary_literal(n: &Node<'_>, analysis: &Analysis<'_>, ctx: &Context<'_>) -> bool {
    if is_numeric_with_sign(n, ctx) {
        return true;
    }
    let Some(parent) = analysis.parent_of(n) else { return false };
    let Some(call) = parent.as_call_node() else { return false };
    is_operator_method(call.name().as_slice())
        && call.message_loc().is_some_and(|l| l.span().start == call.as_node().span().start)
}

/// RuboCop-AST's `NumericNode#sign?`: the literal's own source starts with `+`/`-`.
fn is_numeric_with_sign(n: &Node<'_>, ctx: &Context<'_>) -> bool {
    matches!(
        n.kind(),
        NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::RationalNode
            | NodeKind::ImaginaryNode
    ) && matches!(ctx.text(n.span()).first(), Some(b'+' | b'-'))
}

/// RuboCop's `syntax_like_method_call?`.
fn syntax_like_method_call(target: &Target<'_>) -> bool {
    is_implicit_call(target) || is_operator_method(target.method_name())
}

fn is_implicit_call(target: &Target<'_>) -> bool {
    match target {
        Target::Call(c) => c.message_loc().is_none() && c.name().as_slice() == b"call",
        Target::Yield(_) => false,
    }
}

/// RuboCop's `method_call_before_constant_resolution?`.
fn method_call_before_constant_resolution(target: &Target<'_>, analysis: &Analysis<'_>) -> bool {
    analysis
        .logical_parent_of(&target.as_node())
        .is_some_and(|p| p.kind() == NodeKind::ConstantPathNode)
}

fn is_camel_case(name: &[u8]) -> bool {
    name.first().is_some_and(u8::is_ascii_uppercase)
}

/// RuboCop's `inside_string_interpolation?`.
fn inside_string_interpolation(target: &Target<'_>, analysis: &Analysis<'_>) -> bool {
    let mut cur = target.as_node();
    while let Some(parent) = analysis.parent_of(&cur) {
        if parent.kind() == NodeKind::InterpolatedStringNode {
            return true;
        }
        cur = parent;
    }
    false
}

/// RuboCop's `parentheses_at_the_end_of_multiline_call?`.
fn parens_at_end_of_multiline_call(target: &Target<'_>, ctx: &Context<'_>) -> bool {
    if ctx.is_single_line(target.as_node().span()) {
        return false;
    }
    let Some(open) = target.opening() else { return false };
    let line = ctx.line_col(open.start).line;
    let text = ctx.line_text(line);
    let trimmed = trim_trailing_space(text);
    trimmed.ends_with(b"(")
}

fn trim_trailing_space(text: &[u8]) -> &[u8] {
    let mut end = text.len();
    while end > 0 && (text[end - 1] == b' ' || text[end - 1] == b'\t') {
        end -= 1;
    }
    &text[..end]
}
