//! `Lint/Void`, ported from RuboCop's `lib/rubocop/cop/lint/void.rb`.
//!
//! Upstream drives this cop through five whitequark visitor hooks
//! (`on_block`/`on_numblock`/`on_itblock`, `on_begin`/`on_kwbegin`,
//! `on_ensure`) plus a recursive `check_expression` helper that special-cases
//! `if`/`case`/`case_match` branches. Two AST facts make a literal, hook-based
//! port awkward here:
//!
//! - whitequark elides a `begin` wrapper for a single implicit statement, but
//!   Prism's `StatementsNode` is always present; conversely whitequark's
//!   *explicit* `begin...end`/parenthesised grouping is a distinct `:begin`
//!   node that is **never** elided (`alias on_kwbegin on_begin` fires
//!   regardless of statement count). Prism spells both shapes as `BeginNode`
//!   and `ParenthesesNode` respectively, transparent wrappers around a body
//!   that itself may be absent, a single raw node, or a `StatementsNode`.
//! - `check_begin`'s "is this begin's *own* position void" test
//!   (`in_void_context?`) only ever answers `true` for a `block`/`def`/`for`/
//!   `ensure` *direct* parent; Prism's `CallNode` owns the `BlockNode` (KIT:
//!   "block wraps the call in whitequark; in Prism the `BlockNode` is the
//!   call's `block()`"), so the method-name check that decides a block's
//!   void-ness has to happen at the `CallNode`, one level above where
//!   whitequark would ask it.
//!
//! This port therefore does not use `enter`/`leave` node-kind subscription at
//! all (`kinds: &[]`): it walks the tree itself from `file_start`, computing
//! "is my body a void context" locally at every `DefNode`/`CallNode`-with-
//! block/`ForNode`/`EnsureNode`, and transparently unwrapping any chain of
//! `BeginNode`/`ParenthesesNode` wrappers around a single statement before
//! deciding whether the *last* statement of that body is exempt (returned,
//! not discarded). Every other node kind (`when`/`in`/`else`/`if`-then/
//! `rescue`/top level) always answers `void_context: false`, matching
//! whitequark's `parent.respond_to?(:void_context?)` guard, and is reached
//! generically whenever the walk arrives at a bare `StatementsNode`.
//!
//! Known, fixture-uncovered simplification: whitequark's kwbegin
//! transparency can, in principle, reach all the way up through a
//! `BeginNode`/`ParenthesesNode` chain to a name-dependent `initialize`/
//! setter `def` or an `each`/`tap` block (e.g. `def initialize; begin; 1; 2;
//! end; end`). Because the walk only knows a node's *kind* once it is
//! reached generically (not through its own container), such a case is
//! treated as `void_context: false` instead of inheriting the `def`'s or
//! block's own void-ness. No fixture exercises this; whitequark's kwbegin
//! (or parenthesised) body used as the *entire, single-statement* body of a
//! `def`/block is exotic style.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, StatementsNode};
use ruby_ast::{ext, for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

const BINARY_OPERATORS: &[&[u8]] =
    &[b"*", b"/", b"%", b"+", b"-", b"==", b"===", b"!=", b"<", b">", b"<=", b">=", b"<=>"];
const UNARY_OPERATORS: &[&[u8]] = &[b"+@", b"-@", b"~", b"!"];
const NONMUTATING_METHODS_WITH_BANG_VERSION: &[&[u8]] = &[
    b"capitalize",
    b"chomp",
    b"chop",
    b"compact",
    b"delete_prefix",
    b"delete_suffix",
    b"downcase",
    b"encode",
    b"flatten",
    b"gsub",
    b"lstrip",
    b"merge",
    b"next",
    b"reject",
    b"reverse",
    b"rotate",
    b"rstrip",
    b"scrub",
    b"select",
    b"shuffle",
    b"slice",
    b"sort",
    b"sort_by",
    b"squeeze",
    b"strip",
    b"sub",
    b"succ",
    b"swapcase",
    b"tr",
    b"tr_s",
    b"transform_values",
    b"unicode_normalize",
    b"uniq",
    b"upcase",
];
const METHODS_REPLACEABLE_BY_EACH: &[&[u8]] = &[b"collect", b"map"];

/// How a container's *single*, un-wrapped statement (no `begin`/parens
/// involved) is checked -- whitequark's separate `on_block`/`on_ensure`
/// direct dispatch, which differs in whether `check_void_op` runs at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SingleFlavor {
    /// No direct hook exists for this container (`def`, `for`): a bare
    /// single statement is simply never checked.
    None,
    /// `on_block`'s direct path: `check_void_op` (unsuppressed) then
    /// `check_expression`.
    Block,
    /// `on_ensure`'s direct path: `check_expression` only.
    Ensure,
}

/// Possible use of operator/literal/variable/lambda/proc/nonmutating-method
/// in void context.
#[derive(Debug, Clone)]
pub struct Void {
    check_for_methods_with_no_side_effects: bool,
    /// Whether the nearest enclosing block-with-a-body (climbing through
    /// anything, matching `each_ancestor(:any_block)`) is an `each` call.
    inside_each_block: bool,
    /// Whether the nearest enclosing `def`/`defs` is a setter
    /// (`assignment_method?`) -- suppresses autocorrection of a checked
    /// value even when it isn't the method's own (exempt) return value.
    in_setter_def: bool,
}

impl Rule for Void {
    const META: RuleMeta = RuleMeta {
        name: "Lint/Void",
        department: Department::Lint,
        summary: "Checks for operators, literals, lambdas, and procs used in void context.",
        explanation: "\
Checks for operators, variables, literals, lambda, proc and nonmutating
methods used in void context.

`each` blocks are allowed to prevent false positives. For example, the
expression inside the `each` block below is not void, especially when the
receiver is an `Enumerator`:

```ruby
enumerator = [1, 2, 3].filter
enumerator.each { |item| item >= 2 } #=> [2, 3]
```

The last expression in an assignment method definition such as `def foo=(arg)`
is not flagged: Ruby discards it (the method returns its argument), but the
method can still be called directly and its return value relied upon, so
flagging it would be a false positive for this lint.

A constant used in a void context is flagged but not autocorrected, since
referencing a constant can trigger autoloading side effects (e.g. forcing a
file to load before a monkey-patch), so removing it may change behavior.

```ruby
# bad
def some_method
  some_num * 10
  do_something
end

# good
def some_method
  do_something
  some_num * 10
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[ConfigOption {
            name: "CheckForMethodsWithNoSideEffects",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Whether to check for non-mutating methods that could be replaced by their \
                  mutating counterpart, or a call to `#each`.",
        }],
        blind_spots: "\
A whitequark kwbegin/parenthesised-grouping body used as the *entire,
single-statement* body of a `def`/block (e.g. `def initialize; begin; 1; 2;
end; end`) is treated as a non-void context instead of transparently
inheriting the `def`'s/block's own void-ness, since the walk only knows a
node's kind once reached generically, not through its own container. No
fixture exercises this exotic style.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            check_for_methods_with_no_side_effects: options
                .bool("CheckForMethodsWithNoSideEffects"),
            inside_each_block: false,
            in_setter_def: false,
        })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.inside_each_block = false;
        self.in_setter_def = false;
        let root = ctx.parsed().root();
        self.walk(ctx, &root);
    }
}

impl Void {
    /// Visits every node in the tree once, intercepting the handful of
    /// kinds that can establish or carry a void context (`DefNode`,
    /// `CallNode` with a block, `ForNode`, `EnsureNode`, `BeginNode`,
    /// `ParenthesesNode`, `StatementsNode`) and otherwise recursing
    /// generically. RuboCop's `on_begin`/`on_kwbegin`/`on_block`/`on_ensure`
    /// fire the same way regardless of nesting depth or reachability; this
    /// walk reproduces that by visiting unconditionally rather than only
    /// through `check_expression`'s recursion.
    #[allow(clippy::too_many_lines)]
    fn walk(&mut self, ctx: &mut Context<'_>, node: &Node<'_>) {
        match node.kind() {
            NodeKind::DefNode => {
                let def = node.as_def_node().expect("kind matched");
                if let Some(receiver) = def.receiver() {
                    self.walk(ctx, &receiver);
                }
                if let Some(params) = def.parameters() {
                    self.walk(ctx, &params.as_node());
                }
                let name = def.name();
                let name = name.as_slice();
                let is_setter = is_assignment_method(name);
                let void_context = name == b"initialize" || is_setter;
                let prev_setter = self.in_setter_def;
                self.in_setter_def = is_setter;
                self.process_container(
                    ctx,
                    def.body(),
                    void_context,
                    is_setter,
                    SingleFlavor::None,
                );
                self.in_setter_def = prev_setter;
            }
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                if let Some(receiver) = call.receiver() {
                    self.walk(ctx, &receiver);
                }
                if let Some(args) = call.arguments() {
                    for arg in &args.arguments() {
                        self.walk(ctx, &arg);
                    }
                }
                if let Some(block) = call.block() {
                    match block.as_block_node() {
                        Some(block_node) => {
                            if let Some(params) = block_node.parameters() {
                                self.walk(ctx, &params);
                            }
                            let name = call.name();
                            let name = name.as_slice();
                            let is_each = name == b"each";
                            let is_tap = name == b"tap";
                            let prev_each = self.inside_each_block;
                            self.inside_each_block = is_each;
                            if is_each {
                                self.process_container(
                                    ctx,
                                    block_node.body(),
                                    true,
                                    false,
                                    SingleFlavor::None,
                                );
                            } else if is_tap {
                                self.process_container(
                                    ctx,
                                    block_node.body(),
                                    true,
                                    false,
                                    SingleFlavor::Block,
                                );
                            } else {
                                self.process_container(
                                    ctx,
                                    block_node.body(),
                                    false,
                                    false,
                                    SingleFlavor::None,
                                );
                            }
                            self.inside_each_block = prev_each;
                        }
                        None => self.walk(ctx, &block),
                    }
                }
            }
            NodeKind::ForNode => {
                let for_node = node.as_for_node().expect("kind matched");
                self.walk(ctx, &for_node.index());
                self.walk(ctx, &for_node.collection());
                self.process_container(
                    ctx,
                    for_node.statements().map(|s| s.as_node()),
                    true,
                    false,
                    SingleFlavor::None,
                );
            }
            NodeKind::EnsureNode => {
                let ensure = node.as_ensure_node().expect("kind matched");
                self.process_container(
                    ctx,
                    ensure.statements().map(|s| s.as_node()),
                    true,
                    false,
                    SingleFlavor::Ensure,
                );
            }
            NodeKind::BeginNode => {
                let begin = node.as_begin_node().expect("kind matched");
                let pure = begin.rescue_clause().is_none()
                    && begin.else_clause().is_none()
                    && begin.ensure_clause().is_none();
                if pure {
                    self.process_container(
                        ctx,
                        begin.statements().map(|s| s.as_node()),
                        false,
                        false,
                        SingleFlavor::None,
                    );
                } else {
                    // The body before `rescue`/`ensure`/`else` is never the
                    // structurally last child of this construct (the
                    // rescue/else/ensure clause always is), so it is never a
                    // void context, matching `in_void_context?`'s
                    // "am I my parent's last child" guard.
                    if let Some(stmts) = begin.statements() {
                        let list: Vec<Node<'_>> = stmts.body().iter().collect();
                        self.process_begin_flavor(ctx, &list, false, false);
                        for stmt in &list {
                            self.walk(ctx, stmt);
                        }
                    }
                    if let Some(rescue) = begin.rescue_clause() {
                        self.walk(ctx, &rescue.as_node());
                    }
                    if let Some(else_clause) = begin.else_clause() {
                        self.walk(ctx, &else_clause.as_node());
                    }
                    if let Some(ensure) = begin.ensure_clause() {
                        self.walk(ctx, &ensure.as_node());
                    }
                }
            }
            NodeKind::ParenthesesNode => {
                let parens = node.as_parentheses_node().expect("kind matched");
                self.process_container(ctx, parens.body(), false, false, SingleFlavor::None);
            }
            NodeKind::StatementsNode => {
                let stmts = node.as_statements_node().expect("kind matched");
                self.process_container(
                    ctx,
                    Some(stmts.as_node()),
                    false,
                    false,
                    SingleFlavor::None,
                );
            }
            _ => for_each_child(node, |child| self.walk(ctx, child)),
        }
    }

    /// Resolves a container's body (transparently unwrapping any chain of
    /// single-statement `BeginNode`/`ParenthesesNode` wrappers, matching
    /// whitequark's kwbegin/parens being distinct from an elidable implicit
    /// `begin`), applies the right flavor of checking, then keeps walking
    /// every resulting statement so nested containers are still found.
    fn process_container(
        &mut self,
        ctx: &mut Context<'_>,
        body: Option<Node<'_>>,
        void_context: bool,
        setter_method: bool,
        flavor: SingleFlavor,
    ) {
        let resolved = resolve_body(body);
        if resolved.list.is_empty() {
            return;
        }
        if resolved.list.len() >= 2 || resolved.peeled {
            self.process_begin_flavor(ctx, &resolved.list, void_context, setter_method);
        } else {
            let stmt = resolved.list[0];
            match flavor {
                SingleFlavor::Block => {
                    Self::check_void_op(ctx, &stmt, false);
                    self.check_expression(ctx, &stmt, false);
                }
                SingleFlavor::Ensure => self.check_expression(ctx, &stmt, false),
                SingleFlavor::None => {}
            }
        }
        for stmt in &resolved.list {
            self.walk(ctx, stmt);
        }
    }

    /// RuboCop's `check_begin`: every statement but the last is always
    /// checked; the last is checked only when this list's own position is a
    /// void context, unless it is exempt (inside an `each` block, or the
    /// body of a setter method).
    fn process_begin_flavor(
        &mut self,
        ctx: &mut Context<'_>,
        list: &[Node<'_>],
        void_context: bool,
        setter_method: bool,
    ) {
        let pop_last = !void_context || self.inside_each_block || setter_method;
        let checked_len = if pop_last { list.len().saturating_sub(1) } else { list.len() };
        let suppress_op = self.inside_each_block;
        for stmt in &list[..checked_len] {
            Self::check_void_op(ctx, stmt, suppress_op);
            self.check_expression(ctx, stmt, false);
        }
    }

    /// RuboCop's `check_expression`.
    fn check_expression(&mut self, ctx: &mut Context<'_>, node: &Node<'_>, branch_context: bool) {
        match node.kind() {
            NodeKind::IfNode => {
                let if_node = node.as_if_node().expect("kind matched");
                if let Some(body) = single_unwrap(if_node.statements()) {
                    self.check_void_expression_nodes(ctx, &body, true);
                }
            }
            NodeKind::UnlessNode => {
                let unless_node = node.as_unless_node().expect("kind matched");
                if let Some(body) = single_unwrap(unless_node.statements()) {
                    self.check_void_expression_nodes(ctx, &body, true);
                }
            }
            NodeKind::CaseNode => {
                let case_node = node.as_case_node().expect("kind matched");
                for cond in &case_node.conditions() {
                    if let Some(when) = cond.as_when_node() {
                        if let Some(body) = single_unwrap(when.statements()) {
                            self.check_expression(ctx, &body, true);
                        }
                    }
                }
                if let Some(else_clause) = case_node.else_clause() {
                    if let Some(body) = single_unwrap(else_clause.statements()) {
                        self.check_expression(ctx, &body, true);
                    }
                }
            }
            NodeKind::CaseMatchNode => {
                let case_node = node.as_case_match_node().expect("kind matched");
                for cond in &case_node.conditions() {
                    if let Some(in_node) = cond.as_in_node() {
                        if let Some(body) = single_unwrap(in_node.statements()) {
                            self.check_expression(ctx, &body, true);
                        }
                    }
                }
                if let Some(else_clause) = case_node.else_clause() {
                    if let Some(body) = single_unwrap(else_clause.statements()) {
                        self.check_expression(ctx, &body, true);
                    }
                }
            }
            _ => self.check_void_expression_nodes(ctx, node, branch_context),
        }
    }

    /// RuboCop's `check_void_expression_nodes`.
    fn check_void_expression_nodes(
        &mut self,
        ctx: &mut Context<'_>,
        node: &Node<'_>,
        branch_context: bool,
    ) {
        self.check_literal(ctx, node, branch_context);
        self.check_var(ctx, node, branch_context);
        self.check_self(ctx, node, branch_context);
        self.check_void_expression(ctx, node, branch_context);
        if self.check_for_methods_with_no_side_effects {
            Self::check_nonmutating(ctx, node);
        }
    }

    /// RuboCop's `check_void_op`.
    fn check_void_op(ctx: &mut Context<'_>, node: &Node<'_>, suppress: bool) {
        let node = peel_parens(*node);
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let name = name.as_slice();
        let is_unary = UNARY_OPERATORS.contains(&name);
        if !is_unary && !BINARY_OPERATORS.contains(&name) {
            return;
        }
        let has_args = call.arguments().is_some_and(|a| !a.arguments().is_empty());
        if !is_unary && call.call_operator_loc().is_some() && !has_args {
            return;
        }
        if suppress {
            return;
        }
        let selector_span = call.message_loc().map_or_else(|| node.span(), |l| l.span());
        let op = String::from_utf8_lossy(name).into_owned();
        let message = format!("Operator `{op}` used in void context.");
        let fix = autocorrect_void_op(ctx, &call, has_args);
        ctx.report_with_fix(&Self::META, selector_span, message, fix);
    }

    /// RuboCop's `check_literal`.
    fn check_literal(&mut self, ctx: &mut Context<'_>, node: &Node<'_>, branch_context: bool) {
        if !entirely_literal(node) {
            return;
        }
        if matches!(
            node.kind(),
            NodeKind::XStringNode
                | NodeKind::InterpolatedXStringNode
                | NodeKind::RangeNode
                | NodeKind::NilNode
        ) {
            return;
        }
        let span = node.span();
        let text = String::from_utf8_lossy(ctx.text(span)).into_owned();
        let message = format!("Literal `{text}` used in void context.");
        self.report_maybe_fixed(ctx, span, message, node, branch_context);
    }

    /// RuboCop's `check_var`.
    fn check_var(&mut self, ctx: &mut Context<'_>, node: &Node<'_>, branch_context: bool) {
        let is_const =
            matches!(node.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode);
        let is_var = matches!(
            node.kind(),
            NodeKind::InstanceVariableReadNode
                | NodeKind::GlobalVariableReadNode
                | NodeKind::ClassVariableReadNode
                | NodeKind::LocalVariableReadNode
                | NodeKind::SourceEncodingNode
                | NodeKind::SourceLineNode
                | NodeKind::SourceFileNode
        );
        if !is_const && !is_var {
            return;
        }
        let span = node.span();
        let name = String::from_utf8_lossy(ctx.text(span)).into_owned();
        let message = if is_const {
            format!("Constant `{name}` used in void context.")
        } else {
            format!("Variable `{name}` used in void context.")
        };
        self.report_maybe_fixed(ctx, span, message, node, branch_context);
    }

    /// RuboCop's `check_self`.
    fn check_self(&mut self, ctx: &mut Context<'_>, node: &Node<'_>, branch_context: bool) {
        if node.kind() != NodeKind::SelfNode {
            return;
        }
        self.report_maybe_fixed(
            ctx,
            node.span(),
            "`self` used in void context.".to_string(),
            node,
            branch_context,
        );
    }

    /// RuboCop's `check_void_expression`: a `defined?` call, or a lambda/proc
    /// literal (`-> { }`, `lambda { }`, `proc { }`, `Proc.new { }`).
    fn check_void_expression(
        &mut self,
        ctx: &mut Context<'_>,
        node: &Node<'_>,
        branch_context: bool,
    ) {
        let is_lambda_or_proc = node.kind() == NodeKind::LambdaNode
            || node.as_call_node().is_some_and(|c| ext::is_lambda_or_proc(&c));
        if node.kind() != NodeKind::DefinedNode && !is_lambda_or_proc {
            return;
        }
        let span = node.span();
        let text = String::from_utf8_lossy(ctx.text(span)).into_owned();
        let message = format!("`{text}` used in void context.");
        self.report_maybe_fixed(ctx, span, message, node, branch_context);
    }

    /// RuboCop's `check_nonmutating`.
    fn check_nonmutating(ctx: &mut Context<'_>, node: &Node<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let name = name.as_slice();
        let replaceable_by_each = METHODS_REPLACEABLE_BY_EACH.contains(&name);
        if !replaceable_by_each && !NONMUTATING_METHODS_WITH_BANG_VERSION.contains(&name) {
            return;
        }
        let method = String::from_utf8_lossy(name).into_owned();
        let suggestion =
            if replaceable_by_each { "each".to_string() } else { format!("{method}!") };
        let message =
            format!("Method `#{method}` used in void context. Did you mean `#{suggestion}`?");
        let selector_span = call.message_loc().map_or_else(|| node.span(), |l| l.span());
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(selector_span, suggestion.into_bytes())],
        };
        ctx.report_with_fix(&Self::META, node.span(), message, fix);
    }

    /// Shared reporting tail for `check_literal`/`check_var`/`check_self`/
    /// `check_void_expression`: report with a fix, or bare, depending on
    /// `autocorrect_void_expression`'s guards.
    fn report_maybe_fixed(
        &self,
        ctx: &mut Context<'_>,
        span: Span,
        message: String,
        node: &Node<'_>,
        branch_context: bool,
    ) {
        match self.autocorrect_void_expression(ctx, node, branch_context) {
            Some(fix) => ctx.report_with_fix(&Self::META, span, message, fix),
            None => ctx.report(&Self::META, span, message),
        }
    }

    /// RuboCop's `autocorrect_void_expression`.
    fn autocorrect_void_expression(
        &self,
        ctx: &Context<'_>,
        node: &Node<'_>,
        branch_context: bool,
    ) -> Option<Fix> {
        if matches!(node.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode) {
            return None;
        }
        if branch_context || self.in_setter_def {
            return None;
        }
        let span = ctx.with_surrounding_space(node.span(), Side::Left, true, false);
        Some(Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] })
    }
}

/// The result of transparently unwrapping any chain of single-statement
/// `BeginNode`/`ParenthesesNode` wrappers around a container's body.
struct Resolved<'pr> {
    list: Vec<Node<'pr>>,
    /// Whether at least one `BeginNode`/`ParenthesesNode` layer was peeled
    /// through -- whitequark's kwbegin/parens are never elided, so they
    /// trigger `check_begin` even around a single statement.
    peeled: bool,
}

fn resolve_body(mut body: Option<Node<'_>>) -> Resolved<'_> {
    let mut peeled = false;
    loop {
        let list = stmt_list(body);
        if list.len() == 1 {
            let only = list[0];
            if let Some(begin) = only.as_begin_node() {
                if begin.rescue_clause().is_none()
                    && begin.else_clause().is_none()
                    && begin.ensure_clause().is_none()
                {
                    body = begin.statements().map(|s| s.as_node());
                    peeled = true;
                    continue;
                }
            } else if let Some(parens) = only.as_parentheses_node() {
                body = parens.body();
                peeled = true;
                continue;
            }
        }
        return Resolved { list, peeled };
    }
}

/// A container's body as a flat list: a `StatementsNode`'s own children, a
/// bare single node treated as a one-element list, or nothing.
fn stmt_list(body: Option<Node<'_>>) -> Vec<Node<'_>> {
    match body {
        None => Vec::new(),
        Some(node) => match node.as_statements_node() {
            Some(stmts) => stmts.body().iter().collect(),
            None => vec![node],
        },
    }
}

/// A `StatementsNode`'s sole statement, or `None` if it holds zero or
/// several -- rubocop-ast's raw, un-wrapped `when`/`in`/`else`/`if`-then body
/// for a single statement.
fn single_unwrap(stmts: Option<StatementsNode<'_>>) -> Option<Node<'_>> {
    let stmts = stmts?;
    let body = stmts.body();
    if body.len() == 1 {
        body.first()
    } else {
        None
    }
}

/// `check_void_op`'s `node = node.children.first while node&.begin_type?`:
/// peels through nested parentheses (always a distinct, never-elided
/// `:begin` node in whitequark), taking the first inner statement each time.
fn peel_parens(mut node: Node<'_>) -> Node<'_> {
    while let Some(parens) = node.as_parentheses_node() {
        let inner = match parens.body() {
            Some(b) => match b.as_statements_node() {
                Some(stmts) => stmts.body().first(),
                None => Some(b),
            },
            None => None,
        };
        match inner {
            Some(n) => node = n,
            None => break,
        }
    }
    node
}

/// `MethodIdentifierPredicates#assignment_method?`.
fn is_assignment_method(name: &[u8]) -> bool {
    name.ends_with(b"=") && !matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=")
}

/// `Node#entirely_literal?`.
fn entirely_literal(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::ArrayNode => {
            let array = node.as_array_node().expect("kind matched");
            array.elements().iter().all(|e| entirely_literal(&e))
        }
        NodeKind::HashNode => {
            let hash = node.as_hash_node().expect("kind matched");
            hash.elements().iter().all(|el| {
                if let Some(assoc) = el.as_assoc_node() {
                    entirely_literal(&assoc.key()) && entirely_literal(&assoc.value())
                } else if let Some(splat) = el.as_assoc_splat_node() {
                    splat.value().is_none_or(|v| entirely_literal(&v))
                } else {
                    false
                }
            })
        }
        NodeKind::CallNode => {
            let call = node.as_call_node().expect("kind matched");
            call.name().as_slice() == b"freeze"
                && call.receiver().is_some_and(|r| entirely_literal(&r))
        }
        _ => is_basic_literal_kind(node.kind()),
    }
}

/// `Node::LITERALS` (minus `array`/`hash`, handled recursively above).
fn is_basic_literal_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
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

/// RuboCop's `autocorrect_void_op`.
fn autocorrect_void_op(ctx: &Context<'_>, call: &CallNode<'_>, has_args: bool) -> Fix {
    let node_span = call.as_node().span();
    if !has_args {
        let receiver_span = call.receiver().map_or(node_span, |r| r.span());
        let receiver_text = ctx.text(receiver_span).to_vec();
        return Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(node_span, receiver_text)],
        };
    }
    let mut edits = Vec::new();
    if let Some(dot) = call.call_operator_loc() {
        edits.push(Edit::delete(dot.span()));
    }
    let selector_span = call.message_loc().map_or(node_span, |l| l.span());
    let expanded = ctx.with_surrounding_space(selector_span, Side::Both, false, false);
    edits.push(Edit::replace(expanded, b"\n".to_vec()));
    Fix { applicability: Applicability::Safe, edits }
}
