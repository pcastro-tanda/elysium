//! `Rails/UniqBeforePluck`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/uniq_before_pluck.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `distinct` before `pluck`.";

/// Prefer `distinct` before `pluck`.
#[derive(Debug, Clone)]
pub struct UniqBeforePluck {
    /// `EnforcedStyle: conservative` only flags a `pluck` called on a
    /// constant.
    conservative: bool,
}

impl Rule for UniqBeforePluck {
    const META: RuleMeta = RuleMeta {
        name: "Rails/UniqBeforePluck",
        department: Department::Rails,
        summary: "Prefer the use of uniq or distinct before pluck.",
        explanation: "Prefer the use of `distinct` before `pluck` instead of `uniq` after \
                      `pluck`: the database removes the duplicates rather than Ruby.\n\nThe \
                      autocorrection is unsafe because `uniq` after `pluck` may be applied to \
                      an Array rather than a relation, and `distinct` before `pluck` can \
                      change how the query is ordered.\n\n```ruby\n# bad\nModel.pluck(:id).\
                      uniq\n\n# good\nModel.distinct.pluck(:id)\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("conservative"),
            allowed: &["conservative", "aggressive"],
            doc: "`conservative` only checks `pluck` called on a constant; `aggressive` checks \
                  every receiver.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { conservative: options.style("EnforcedStyle")? == "conservative" })
    }

    fn enter(&mut self, uniq: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(uniq_call) = uniq.as_call_node() else { return };
        if uniq_call.name().as_slice() != b"uniq" || uniq_call.is_safe_navigation() {
            return;
        }
        // `[!^any_block ...]`: the node is neither the call of a block nor
        // a block's lone body statement.
        if uniq_call.block().is_some_and(|block| block.as_block_node().is_some()) {
            return;
        }
        if is_lone_block_body(uniq, ctx) {
            return;
        }
        let Some(pluck) = uniq_call.receiver() else { return };
        let Some(pluck_call) = pluck.as_call_node() else { return };
        if pluck_call.name().as_slice() != b"pluck"
            || pluck_call.is_safe_navigation()
            || pluck_call.block().is_some_and(|block| block.as_block_node().is_some())
        {
            return;
        }
        if self.conservative
            && !pluck_call.receiver().is_some_and(|r| {
                r.as_constant_read_node().is_some() || r.as_constant_path_node().is_some()
            })
        {
            return;
        }
        let Some(selector) = uniq_call.message_loc() else { return };
        let pluck_end = pluck_call
            .closing_loc()
            .or_else(|| pluck_call.message_loc())
            .map_or(pluck.span().end, |loc| loc.span().end);
        let mut edits = vec![Edit::delete(Span::new(pluck_end, selector.span().end))];
        match pluck_call.call_operator_loc() {
            Some(dot) => edits.push(Edit::insert(dot.span().start, b".distinct".to_vec())),
            None => edits.push(Edit::insert(pluck.span().start, b"distinct.".to_vec())),
        }
        ctx.report_with_fix(
            &Self::META,
            selector.span(),
            MSG,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// Whether the node being entered is the only statement of a block (or
/// lambda) body, which `parser` parents directly to the block.
fn is_lone_block_body(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let ancestors = ctx.ancestors();
    let [.., grandparent, parent] = ancestors else { return false };
    parent.kind == NodeKind::StatementsNode
        && parent.span == node.span()
        && matches!(grandparent.kind, NodeKind::BlockNode | NodeKind::LambdaNode)
}
