//! `Style/MethodCallWithoutArgsParentheses`, ported from RuboCop's
//! `lib/rubocop/cop/style/method_call_without_args_parentheses.rb` plus the
//! `AllowedMethods`/`AllowedPattern` mixins it includes.
//!
//! # `same_name_assignment?`
//!
//! Upstream walks `node.each_ancestor(*AST::Node::ASSIGNMENTS)`, where
//! `ASSIGNMENTS` is `lvasgn`/`ivasgn`/`cvasgn`/`gvasgn`/`casgn`/`masgn` (the
//! "equals" forms) plus `op_asgn`/`or_asgn`/`and_asgn` (the shorthand forms),
//! skipping any shorthand ancestor whose `lhs` is a `send` (`obj.method ||=
//! func()`: the receiver-ful write is itself a call, so the guard never
//! protects it). Prism gives each of those a disjoint node kind instead of
//! one generic shape, and -- critically -- the shorthand-on-a-call/index
//! forms (`CallOrWriteNode`, `IndexOperatorWriteNode`, ...) *always* have a
//! call-shaped target, so they would always be skipped anyway: this port
//! simply never subscribes to those kinds, which is equivalent to always
//! skipping them.
//!
//! Since [`linter::Context::ancestors`] only carries `(kind, span)` (no live
//! node handle, so no `.name()`/`.lefts()`), the remaining ancestor kinds
//! that *do* matter (the plain variable/constant writes, their shorthand
//! forms, and `MultiWriteNode`) are tracked on this rule's own stack --
//! pushed with the name(s) they bind when entered, popped when left --
//! following the `constant_definition_in_block.rs` pattern instead of
//! re-deriving them from `ancestors()`.
//!
//! # The `it`-in-block guard
//!
//! Whitequark gives a block distinct node types depending on its implicit
//! parameters (`block` for explicit/no params, `numblock` for `_1`,
//! `itblock` for a bare `it` reference), and `parenthesized_it_method_in_block?`
//! only applies its param-shape guard when the ancestor block is a plain
//! `block_type?` node. Prism instead gives every block literal one
//! [`NodeKind::BlockNode`] and expresses the param shape entirely through
//! its `parameters` field: `None` (bare, matching both a truly empty block
//! and an implicit-`it` one), [`ruby_ast::node::NumberedParametersNode`]
//! (`_1`), or [`ruby_ast::node::BlockParametersNode`] (an explicit `|...|`,
//! empty or not). Upstream's guard reduces to "does the block have an
//! explicit `BlockParametersNode`" either way, so this port tracks just that
//! one bit per enclosing block/lambda, again on its own stack.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::node::MultiWriteNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Do not use parentheses for method calls with no arguments.";

/// One binding contributed by an assignment-shaped ancestor: RuboCop's
/// `same_name_assignment?`/`variable_in_mass_assignment?`.
#[derive(Debug, Clone)]
enum AssignFact {
    /// A plain (or shorthand) single-target write: the bound name.
    Named(Vec<u8>),
    /// A `masgn`: every non-`send`-shaped left/rest/right target's name
    /// (RuboCop-AST's `MlhsNode#assignments` filtered by
    /// `reject(&:send_type?)`).
    Masgn(Vec<Vec<u8>>),
}

/// `Style/MethodCallWithoutArgsParentheses`.
#[derive(Debug, Clone)]
pub struct MethodCallWithoutArgsParentheses {
    /// `AllowedMethods`.
    allowed_methods: Vec<String>,
    /// `AllowedPatterns`, precompiled.
    allowed_patterns: Vec<Regex>,
    /// Own ancestor stack of currently open assignment-shaped nodes,
    /// outermost first. See the module doc.
    assign_stack: Vec<AssignFact>,
    /// Own ancestor stack of currently open `BlockNode`/`LambdaNode`
    /// literals: `true` when the block has an explicit `BlockParametersNode`
    /// (`|...|`, empty or not). See the module doc.
    block_stack: Vec<bool>,
}

impl Rule for MethodCallWithoutArgsParentheses {
    const META: RuleMeta = RuleMeta {
        name: "Style/MethodCallWithoutArgsParentheses",
        department: Department::Style,
        summary: "Do not use parentheses for method calls with no arguments.",
        explanation: "\
This cop's allowed methods can be customized with `AllowedMethods`. By default,
there are no allowed methods.

NOTE: This cop allows the use of `it()` without arguments in blocks, as in
`0.times { it() }`, following `Lint/ItWithoutArgumentsInBlock`.

```ruby
# bad
object.some_method()

# good
object.some_method
```

With `AllowedMethods: []` (default):

```ruby
# bad
object.foo()
```

With `AllowedMethods: [foo]`:

```ruby
# good
object.foo()
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::BlockNode,
            NodeKind::LambdaNode,
            NodeKind::LocalVariableWriteNode,
            NodeKind::LocalVariableOrWriteNode,
            NodeKind::LocalVariableAndWriteNode,
            NodeKind::LocalVariableOperatorWriteNode,
            NodeKind::InstanceVariableWriteNode,
            NodeKind::InstanceVariableOrWriteNode,
            NodeKind::InstanceVariableAndWriteNode,
            NodeKind::InstanceVariableOperatorWriteNode,
            NodeKind::ClassVariableWriteNode,
            NodeKind::ClassVariableOrWriteNode,
            NodeKind::ClassVariableAndWriteNode,
            NodeKind::ClassVariableOperatorWriteNode,
            NodeKind::GlobalVariableWriteNode,
            NodeKind::GlobalVariableOrWriteNode,
            NodeKind::GlobalVariableAndWriteNode,
            NodeKind::GlobalVariableOperatorWriteNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantOrWriteNode,
            NodeKind::ConstantAndWriteNode,
            NodeKind::ConstantOperatorWriteNode,
            NodeKind::ConstantPathWriteNode,
            NodeKind::ConstantPathOrWriteNode,
            NodeKind::ConstantPathAndWriteNode,
            NodeKind::ConstantPathOperatorWriteNode,
            NodeKind::MultiWriteNode,
        ],
        config: &[
            linter::ConfigOption {
                name: "AllowedMethods",
                default: linter::ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method names always allowed to keep empty parentheses.",
            },
            linter::ConfigOption {
                name: "AllowedPatterns",
                default: linter::ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method name regex patterns always allowed to keep empty parentheses.",
            },
        ],
        blind_spots: "\
`AllowedPatterns` entries that fail to compile as a Rust regex are dropped (never match) rather \
than raising a configuration error.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let allowed_methods = options.str_list("AllowedMethods");
        let allowed_patterns = options
            .str_list("AllowedPatterns")
            .iter()
            .filter_map(|pattern| Regex::new(pattern).ok())
            .collect();
        Ok(Self {
            allowed_methods,
            allowed_patterns,
            assign_stack: Vec::new(),
            block_stack: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => self.check_call(node, ctx),
            NodeKind::BlockNode => {
                let has_params = node
                    .as_block_node()
                    .and_then(|b| b.parameters())
                    .is_some_and(|p| p.as_block_parameters_node().is_some());
                self.block_stack.push(has_params);
            }
            NodeKind::LambdaNode => {
                let has_params = node
                    .as_lambda_node()
                    .and_then(|b| b.parameters())
                    .is_some_and(|p| p.as_block_parameters_node().is_some());
                self.block_stack.push(has_params);
            }
            _ => {
                if let Some(fact) = assign_fact(node) {
                    self.assign_stack.push(fact);
                }
            }
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {}
            NodeKind::BlockNode | NodeKind::LambdaNode => {
                self.block_stack.pop();
            }
            _ => {
                if assign_fact(node).is_some() {
                    self.assign_stack.pop();
                }
            }
        }
    }
}

impl MethodCallWithoutArgsParentheses {
    fn check_call(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.arguments().is_some() {
            return;
        }
        // Whitequark treats a `&blk`/`&:sym` block-pass as an *argument*
        // (`node.arguments?` true); Prism instead threads it through
        // `CallNode::block()` as a `BlockArgumentNode`, disjoint from
        // `arguments()`. Match upstream's shape here.
        if call.block().is_some_and(|b| b.as_block_argument_node().is_some()) {
            return;
        }
        let Some(opening) = call.opening_loc() else { return };
        let Some(closing) = call.closing_loc() else { return };
        if ctx.text(closing.span()) != b")" {
            return;
        }
        if Self::ineligible(&call, ctx) {
            return;
        }
        if Self::default_argument(ctx) {
            return;
        }
        let name = call.name();
        let name = name.as_slice();
        if self.allowed_method_name(name) {
            return;
        }
        if self.same_name_assignment(&call, name) {
            return;
        }
        if self.parenthesized_it_method_in_block(&call, name) {
            return;
        }

        let range = Span::new(opening.span().start, closing.span().end);
        if comment_on_lines(ctx, range) {
            return;
        }
        ctx.report_with_fix(
            &Self::META,
            range,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(range)] },
        );
    }

    /// RuboCop's `ineligible_node?`: `camel_case_method? || implicit_call? ||
    /// prefix_not?`.
    fn ineligible(call: &ruby_ast::node::CallNode<'_>, ctx: &Context<'_>) -> bool {
        let name = call.name();
        let name = name.as_slice();
        if name.first().is_some_and(u8::is_ascii_uppercase) {
            return true;
        }
        if name == b"call" && call.message_loc().is_none() {
            return true;
        }
        if call.receiver().is_some() && name == b"!" {
            if let Some(message) = call.message_loc() {
                if ctx.text(message.span()) == b"not" {
                    return true;
                }
            }
        }
        false
    }

    /// RuboCop's `default_argument?`: `node.parent&.optarg_type?`.
    fn default_argument(ctx: &Context<'_>) -> bool {
        ctx.parent().is_some_and(|p| p.kind == NodeKind::OptionalParameterNode)
    }

    /// RuboCop's `allowed_method_name?`.
    fn allowed_method_name(&self, name: &[u8]) -> bool {
        let Ok(text) = std::str::from_utf8(name) else { return false };
        self.allowed_methods.iter().any(|m| m == text)
            || self.allowed_patterns.iter().any(|re| re.is_match(text))
    }

    /// RuboCop's `same_name_assignment?`/`any_assignment?`.
    fn same_name_assignment(&self, call: &ruby_ast::node::CallNode<'_>, name: &[u8]) -> bool {
        if call.receiver().is_some() {
            return false;
        }
        self.assign_stack.iter().any(|fact| match fact {
            AssignFact::Named(bound) => bound.as_slice() == name,
            AssignFact::Masgn(names) => names.iter().any(|n| n.as_slice() == name),
        })
    }

    /// RuboCop's `parenthesized_it_method_in_block?`.
    fn parenthesized_it_method_in_block(
        &self,
        call: &ruby_ast::node::CallNode<'_>,
        name: &[u8],
    ) -> bool {
        if name != b"it" {
            return false;
        }
        let Some(&has_params) = self.block_stack.last() else { return false };
        if has_params {
            return false;
        }
        let has_block_literal = call
            .block()
            .is_some_and(|b| b.as_block_node().is_some() || b.as_lambda_node().is_some());
        call.receiver().is_none() && !has_block_literal
    }
}

/// RuboCop's `contains_comment?`: any comment on any line `span` spans
/// (inclusive of both endpoints), regardless of column.
fn comment_on_lines(ctx: &Context<'_>, span: Span) -> bool {
    let start_line = ctx.line_col(span.start).line;
    let end_line = ctx.line_col(span.end.saturating_sub(1)).line;
    ctx.comments().iter().any(|c| c.line >= start_line && c.line <= end_line)
}

/// Builds the [`AssignFact`] this rule tracks for an assignment-shaped
/// ancestor node, or `None` for any other (untracked) kind.
fn assign_fact(node: &Node<'_>) -> Option<AssignFact> {
    macro_rules! named {
        ($accessor:ident) => {
            node.$accessor().map(|n| AssignFact::Named(n.name().as_slice().to_vec()))
        };
    }
    match node.kind() {
        NodeKind::LocalVariableWriteNode => named!(as_local_variable_write_node),
        NodeKind::LocalVariableOrWriteNode => named!(as_local_variable_or_write_node),
        NodeKind::LocalVariableAndWriteNode => named!(as_local_variable_and_write_node),
        NodeKind::LocalVariableOperatorWriteNode => named!(as_local_variable_operator_write_node),
        NodeKind::InstanceVariableWriteNode => named!(as_instance_variable_write_node),
        NodeKind::InstanceVariableOrWriteNode => named!(as_instance_variable_or_write_node),
        NodeKind::InstanceVariableAndWriteNode => named!(as_instance_variable_and_write_node),
        NodeKind::InstanceVariableOperatorWriteNode => {
            named!(as_instance_variable_operator_write_node)
        }
        NodeKind::ClassVariableWriteNode => named!(as_class_variable_write_node),
        NodeKind::ClassVariableOrWriteNode => named!(as_class_variable_or_write_node),
        NodeKind::ClassVariableAndWriteNode => named!(as_class_variable_and_write_node),
        NodeKind::ClassVariableOperatorWriteNode => named!(as_class_variable_operator_write_node),
        NodeKind::GlobalVariableWriteNode => named!(as_global_variable_write_node),
        NodeKind::GlobalVariableOrWriteNode => named!(as_global_variable_or_write_node),
        NodeKind::GlobalVariableAndWriteNode => named!(as_global_variable_and_write_node),
        NodeKind::GlobalVariableOperatorWriteNode => named!(as_global_variable_operator_write_node),
        NodeKind::ConstantWriteNode => named!(as_constant_write_node),
        NodeKind::ConstantOrWriteNode => named!(as_constant_or_write_node),
        NodeKind::ConstantAndWriteNode => named!(as_constant_and_write_node),
        NodeKind::ConstantOperatorWriteNode => named!(as_constant_operator_write_node),
        NodeKind::ConstantPathWriteNode => node.as_constant_path_write_node().and_then(|n| {
            n.target().name().map(|name| AssignFact::Named(name.as_slice().to_vec()))
        }),
        NodeKind::ConstantPathOrWriteNode => node.as_constant_path_or_write_node().and_then(|n| {
            n.target().name().map(|name| AssignFact::Named(name.as_slice().to_vec()))
        }),
        NodeKind::ConstantPathAndWriteNode => {
            node.as_constant_path_and_write_node().and_then(|n| {
                n.target().name().map(|name| AssignFact::Named(name.as_slice().to_vec()))
            })
        }
        NodeKind::ConstantPathOperatorWriteNode => {
            node.as_constant_path_operator_write_node().and_then(|n| {
                n.target().name().map(|name| AssignFact::Named(name.as_slice().to_vec()))
            })
        }
        NodeKind::MultiWriteNode => {
            node.as_multi_write_node().map(|n| AssignFact::Masgn(masgn_names(&n)))
        }
        _ => None,
    }
}

/// RuboCop-AST's `MlhsNode#assignments` (via `MasgnNode#assignments`)
/// filtered by `reject(&:send_type?)`: every left/rest/right target's own
/// bound name, skipping call-shaped targets (`obj.attr, ... =` /
/// `obj[idx], ... =`) entirely -- they contribute nothing, matching
/// upstream dropping them before the name comparison.
fn masgn_names(masgn: &MultiWriteNode<'_>) -> Vec<Vec<u8>> {
    let mut names = Vec::new();
    for target in &masgn.lefts() {
        collect_target_name(&target, &mut names);
    }
    if let Some(rest) = masgn.rest() {
        collect_target_name(&rest, &mut names);
    }
    for target in &masgn.rights() {
        collect_target_name(&target, &mut names);
    }
    names
}

fn collect_target_name(node: &Node<'_>, out: &mut Vec<Vec<u8>>) {
    match node.kind() {
        NodeKind::LocalVariableTargetNode => {
            if let Some(n) = node.as_local_variable_target_node() {
                out.push(n.name().as_slice().to_vec());
            }
        }
        NodeKind::InstanceVariableTargetNode => {
            if let Some(n) = node.as_instance_variable_target_node() {
                out.push(n.name().as_slice().to_vec());
            }
        }
        NodeKind::ClassVariableTargetNode => {
            if let Some(n) = node.as_class_variable_target_node() {
                out.push(n.name().as_slice().to_vec());
            }
        }
        NodeKind::GlobalVariableTargetNode => {
            if let Some(n) = node.as_global_variable_target_node() {
                out.push(n.name().as_slice().to_vec());
            }
        }
        NodeKind::ConstantTargetNode => {
            if let Some(n) = node.as_constant_target_node() {
                out.push(n.name().as_slice().to_vec());
            }
        }
        NodeKind::ConstantPathTargetNode => {
            if let Some(name) = node.as_constant_path_target_node().and_then(|n| n.name()) {
                out.push(name.as_slice().to_vec());
            }
        }
        NodeKind::SplatNode => {
            if let Some(expr) = node.as_splat_node().and_then(|n| n.expression()) {
                collect_target_name(&expr, out);
            }
        }
        NodeKind::MultiTargetNode => {
            if let Some(n) = node.as_multi_target_node() {
                for target in &n.lefts() {
                    collect_target_name(&target, out);
                }
                if let Some(rest) = n.rest() {
                    collect_target_name(&rest, out);
                }
                for target in &n.rights() {
                    collect_target_name(&target, out);
                }
            }
        }
        // `CallTargetNode`/`IndexTargetNode`: RuboCop-AST's `send_type?`
        // rejects (`obj.attr, ... =` / `obj[idx], ... =`).
        _ => {}
    }
}
