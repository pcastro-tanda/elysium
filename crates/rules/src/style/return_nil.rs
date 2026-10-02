//! `Style/ReturnNil`, ported from RuboCop's
//! `lib/rubocop/cop/style/return_nil.rb`.
//!
//! # Ancestor tracking
//!
//! Before checking the style, upstream walks
//! `return_node.each_ancestor(:block, :any_def)` outward, bailing out of
//! the check entirely (no offense either way) the moment it crosses a
//! chained, argumented block, and stopping the walk (continuing on to the
//! normal style check) at the first `def`/`defs` or `lambda?` block, or at
//! a block owned by `define_method`/`define_singleton_method`. This is the
//! exact same ancestor shape as `Lint/NonLocalExitFromIterator`'s
//! `scoped_node?`/`chained_send?`/`define_method?` walk; this port reuses
//! that file's `Frame` stack approach privately (see its module doc for the
//! `Context::ancestors`-has-no-typed-send-node rationale) instead of
//! walking parent pointers.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::BlockNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `RETURN_MSG`.
const RETURN_MSG: &str = "Use `return` instead of `return nil`.";
/// RuboCop's `RETURN_NIL_MSG`.
const RETURN_NIL_MSG: &str = "Use `return nil` instead of `return`.";

/// One enclosing `any_block`/`any_def` ancestor, innermost last. Copied
/// from `Lint/NonLocalExitFromIterator`.
#[derive(Debug, Clone, Copy)]
enum Frame {
    /// RuboCop's `scoped_node?` (`any_def_type? || lambda?`).
    Scoped,
    /// A block literal that is neither scoped nor a `lambda` call.
    Block {
        /// RuboCop's `!node.argument_list.empty?`.
        has_arguments: bool,
        /// RuboCop's `define_method?(node.send_node)`.
        is_define_method: bool,
        /// RuboCop's `chained_send?(node.send_node)`.
        is_chained: bool,
    },
}

/// Whether a `CallNode` owns a literal block.
fn owns_literal_block(node: &Node<'_>) -> bool {
    node.as_call_node()
        .is_some_and(|call| call.block().is_some_and(|b| b.as_block_node().is_some()))
}

/// RuboCop-AST's `BlockNode#argument_list.empty?`, negated.
fn block_has_arguments(block: &BlockNode<'_>) -> bool {
    match block.parameters() {
        None => false,
        Some(params) => match params.kind() {
            NodeKind::NumberedParametersNode | NodeKind::ItParametersNode => true,
            NodeKind::BlockParametersNode => {
                let Some(block_params) = params.as_block_parameters_node() else { return false };
                block_params.parameters().is_some() || !block_params.locals().is_empty()
            }
            _ => false,
        },
    }
}

/// Use return instead of return nil.
#[derive(Debug, Clone, Default)]
pub struct ReturnNil {
    style_is_return: bool,
    /// Enclosing `any_block`/`any_def` ancestors, innermost last.
    stack: Vec<Frame>,
}

impl ReturnNil {
    /// RuboCop's `each_ancestor` loop: `true` if this `return` must be left
    /// alone regardless of style (RuboCop's `return nil` early exit from
    /// `on_return`, matching a chained, argumented block ancestor).
    fn inside_chained_argumented_block(&self) -> bool {
        for frame in self.stack.iter().rev() {
            match *frame {
                Frame::Scoped | Frame::Block { is_define_method: true, .. } => return false,
                Frame::Block { has_arguments: false, .. } => {}
                Frame::Block { is_chained: true, .. } => return true,
                Frame::Block { .. } => {}
            }
        }
        false
    }
}

impl Rule for ReturnNil {
    const META: RuleMeta = RuleMeta {
        name: "Style/ReturnNil",
        department: Department::Style,
        summary: "Use return instead of return nil.",
        explanation: "Enforces consistency between `return nil` and `return`. This cop is \
            disabled by default. Because there seems to be a perceived semantic difference \
            between `return` and `return nil`. The former can be seen as just halting \
            evaluation, while the latter might be used when the return value is of specific \
            concern.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ReturnNode, NodeKind::CallNode, NodeKind::DefNode, NodeKind::LambdaNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("return"),
            allowed: &["return", "return_nil"],
            doc: "Whether to prefer `return` or `return nil`.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = options.style("EnforcedStyle")?;
        Ok(Self { style_is_return: style == "return", stack: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.stack.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::DefNode | NodeKind::LambdaNode => {
                self.stack.push(Frame::Scoped);
            }
            NodeKind::CallNode => {
                if owns_literal_block(node) {
                    let call = node.as_call_node().expect("kind matched");
                    if call.name().as_slice() == b"lambda" {
                        self.stack.push(Frame::Scoped);
                    } else {
                        let block = call
                            .block()
                            .and_then(|b| b.as_block_node())
                            .expect("owns_literal_block checked");
                        self.stack.push(Frame::Block {
                            has_arguments: block_has_arguments(&block),
                            is_define_method: !call.is_safe_navigation()
                                && matches!(
                                    call.name().as_slice(),
                                    b"define_method" | b"define_singleton_method"
                                ),
                            is_chained: call.receiver().is_some() && !call.is_safe_navigation(),
                        });
                    }
                }
            }
            NodeKind::ReturnNode => self.check(node, ctx),
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::DefNode | NodeKind::LambdaNode => {
                self.stack.pop();
            }
            NodeKind::CallNode if owns_literal_block(node) => {
                self.stack.pop();
            }
            _ => {}
        }
    }
}

impl ReturnNil {
    /// RuboCop's `on_return`.
    fn check(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.inside_chained_argumented_block() {
            return;
        }

        let ret = node.as_return_node().expect("kind matched");
        let args = ret.arguments().map(|a| a.arguments());
        // RuboCop's `return_node?`: `(return)`, no arguments at all.
        let is_bare_return = args.is_none();
        // RuboCop's `return_nil_node?`: `(return nil)`, exactly one `nil` argument.
        let is_return_nil = args
            .as_ref()
            .is_some_and(|a| a.len() == 1 && a.first().is_some_and(|n| n.as_nil_node().is_some()));

        let (correct, message, replacement): (bool, &str, &str) = if self.style_is_return {
            (!is_return_nil, RETURN_MSG, "return")
        } else {
            (!is_bare_return, RETURN_NIL_MSG, "return nil")
        };
        if correct {
            return;
        }

        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(node.span(), replacement.as_bytes().to_vec())],
            },
        );
    }
}
