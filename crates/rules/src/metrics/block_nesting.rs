//! `Metrics/BlockNesting`, ported from RuboCop's
//! `lib/rubocop/cop/metrics/block_nesting.rb`.
//!
//! Upstream walks the whole tree with `each_child_node`, incrementing a
//! running nesting level at each `if`/`case`/`while`/`until`/`for`/`resbody`
//! (and, with `CountBlocks`, each block) and reporting once the level
//! exceeds `Max`. A reported node's own subtree is then exempt from further
//! reports (`ignore_node`/`part_of_ignored_node?` walks up a flagged node's
//! ancestors, which -- since traversal is top-down -- is equivalent to never
//! descending past a node once it has been flagged).
//!
//! whitequark's `if` node type covers both `if` and Prism's separate
//! `UnlessNode`; both get the same elsif/modifier-form treatment here.
//! `while_post`/`until_post` (the `begin...end while`/`until` modifier
//! forms) are Prism's ordinary `WhileNode`/`UntilNode` with
//! `is_begin_modifier` set, and -- unlike `if`/`unless` -- always count,
//! regardless of `CountModifierForms`, since only `IfNode#modifier_form?`
//! consults that option.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};

/// Avoid excessive block nesting.
#[derive(Debug, Clone)]
pub struct BlockNesting {
    max: i64,
    count_blocks: bool,
    count_modifier_forms: bool,
}

impl Rule for BlockNesting {
    const META: RuleMeta = RuleMeta {
        name: "Metrics/BlockNesting",
        department: Department::Metrics,
        summary: "Avoid excessive block nesting.",
        explanation: "Checks for excessive nesting of conditional and looping constructs.\n\n\
            You can configure if blocks are considered using the `CountBlocks` and \
            `CountModifierForms` options. When both are set to `false` (the default) blocks and \
            modifier forms are not counted towards the nesting level. Set them to `true` to \
            include these in the nesting level calculation as well.\n\n\
            The maximum level of nesting allowed is configurable.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[],
        config: &[
            ConfigOption {
                name: "Max",
                default: ConfigDefault::Int(3),
                allowed: &[],
                doc: "Maximum level of block nesting allowed.",
            },
            ConfigOption {
                name: "CountBlocks",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Count `block`s as nesting.",
            },
            ConfigOption {
                name: "CountModifierForms",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Count modifier forms of `if`/`unless` as nesting.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            max: options.int("Max"),
            count_blocks: options.bool("CountBlocks"),
            count_modifier_forms: options.bool("CountModifierForms"),
        })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        check_nesting_level(
            &root,
            None,
            ctx,
            self.max,
            self.count_blocks,
            self.count_modifier_forms,
            0,
        );
    }
}

/// Upstream's `message(max)`.
fn message(max: i64) -> String {
    format!("Avoid more than {max} levels of block nesting.")
}

/// Upstream's `consider_node?`.
fn consider_node(kind: NodeKind, count_blocks: bool) -> bool {
    matches!(
        kind,
        NodeKind::CaseNode
            | NodeKind::CaseMatchNode
            | NodeKind::IfNode
            | NodeKind::UnlessNode
            | NodeKind::WhileNode
            | NodeKind::UntilNode
            | NodeKind::ForNode
            | NodeKind::RescueNode
    ) || (count_blocks && kind == NodeKind::BlockNode)
}

/// Upstream's `count_if_block?`. Non-`if`/`unless` considered nodes always
/// count; `if`/`unless` nodes count unless they are an `elsif` link, with
/// modifier (postfix) forms gated on `CountModifierForms`. A ternary (an
/// `IfNode` with no `if_keyword_loc`) always counts, since
/// `IfNode#modifier_form?` requires the `if`/`unless` keyword and so is
/// always `false` for one.
fn count_if_block(node: &Node<'_>, count_modifier_forms: bool) -> bool {
    if let Some(if_node) = node.as_if_node() {
        let Some(if_keyword) = if_node.if_keyword_loc() else { return true };
        if if_keyword.span().len() == 5 {
            // `elsif`.
            return false;
        }
        if if_node.end_keyword_loc().is_none() {
            return count_modifier_forms;
        }
        true
    } else if let Some(unless_node) = node.as_unless_node() {
        if unless_node.end_keyword_loc().is_none() {
            count_modifier_forms
        } else {
            true
        }
    } else {
        true
    }
}

/// Port of `check_nesting_level`. Once a node is flagged its subtree is
/// never descended further: every node under it would be
/// `part_of_ignored_node?` upstream, and reporting only requires the
/// currently-open message anyway, so no offense there could ever surface.
///
/// `parent` is `node`'s immediate Prism parent. whitequark's `:block`/
/// `:numblock`/`:itblock` node wraps the *whole* `recv.method(args) { ... }`
/// construct, so upstream's `add_offense(node)` for a flagged block covers
/// it end to end; Prism's `BlockNode` is only the `{ ... }`/`do...end` part,
/// but its owning `CallNode`'s span already extends through it, so that is
/// reported instead.
/// `subsequent` is fixed up here: Prism chains multiple `rescue` clauses via
/// `RescueNode::subsequent`, nesting the second clause as a child of the
/// first, whereas whitequark makes every `resbody` a sibling under the
/// enclosing `rescue` node, all sharing the same incoming level. Recursing
/// into `subsequent` with `current_level` (not this clause's own
/// post-increment `level`) reproduces that, and it must still happen even
/// when this clause itself gets flagged: `ignore_node` only ever suppresses
/// a flagged node's own subtree, never its whitequark siblings.
fn check_nesting_level<'pr>(
    node: &Node<'pr>,
    parent: Option<&Node<'pr>>,
    ctx: &mut Context<'_>,
    max: i64,
    count_blocks: bool,
    count_modifier_forms: bool,
    current_level: u32,
) {
    let mut level = current_level;
    let mut reported = false;
    if consider_node(node.kind(), count_blocks) {
        if count_if_block(node, count_modifier_forms) {
            level += 1;
        }
        if i64::from(level) > max {
            let span = if node.kind() == NodeKind::BlockNode {
                parent.map_or_else(|| node.span(), Node::span)
            } else {
                node.span()
            };
            ctx.report(&BlockNesting::META, span, message(max));
            reported = true;
        }
    }

    if let Some(rescue) = node.as_rescue_node() {
        if !reported {
            for exception in &rescue.exceptions() {
                check_nesting_level(
                    &exception,
                    Some(node),
                    ctx,
                    max,
                    count_blocks,
                    count_modifier_forms,
                    level,
                );
            }
            if let Some(reference) = rescue.reference() {
                check_nesting_level(
                    &reference,
                    Some(node),
                    ctx,
                    max,
                    count_blocks,
                    count_modifier_forms,
                    level,
                );
            }
            if let Some(statements) = rescue.statements() {
                check_nesting_level(
                    &statements.as_node(),
                    Some(node),
                    ctx,
                    max,
                    count_blocks,
                    count_modifier_forms,
                    level,
                );
            }
        }
        if let Some(subsequent) = rescue.subsequent() {
            check_nesting_level(
                &subsequent.as_node(),
                parent,
                ctx,
                max,
                count_blocks,
                count_modifier_forms,
                current_level,
            );
        }
        return;
    }

    if reported {
        return;
    }
    for_each_child(node, |child| {
        check_nesting_level(child, Some(node), ctx, max, count_blocks, count_modifier_forms, level);
    });
}
