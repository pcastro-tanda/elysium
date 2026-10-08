//! `Style/KeywordArgumentsMerging`, ported from RuboCop's
//! `lib/rubocop/cop/style/keyword_arguments_merging.rb`.
//!
//! Upstream subscribes to `on_kwsplat` and climbs two ancestors
//! (`node.parent&.parent`) to the enclosing `send`; Prism inserts an extra
//! `ArgumentsNode` layer between a call and its argument list, so this
//! subscribes to the braceless-kwargs wrapper itself
//! ([`ruby_ast::NodeKind::KeywordHashNode`], whitequark's implicit `hash`
//! node) instead and checks its own first element for the `**`-splat --
//! equivalent to upstream's `(hash (kwsplat ...) ...)` pattern requiring the
//! splat to be the hash's first child -- then confirms its parent chain is
//! `CallNode <- ArgumentsNode <- KeywordHashNode`, matching the pattern's
//! `(send _ _ ... (hash ...))` grandparent requirement.
//!
//! A trailing `&block` on the `merge` call (`merge(other, &my_proc)`) is
//! `CallNode::block`, not a member of `arguments` -- unlike whitequark,
//! where it is the argument list's own last, `block_pass_type?` element --
//! so [`has_block_pass`] checks that field directly instead.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Provide additional arguments directly rather than using `merge`.";

/// When passing an existing hash as keyword arguments, provide additional
/// arguments directly rather than using `merge`.
#[derive(Debug, Clone)]
pub struct KeywordArgumentsMerging;

impl Rule for KeywordArgumentsMerging {
    const META: RuleMeta = RuleMeta {
        name: "Style/KeywordArgumentsMerging",
        department: Department::Style,
        summary: "When passing an existing hash as keyword arguments, provide additional arguments directly rather than using `merge`.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::KeywordHashNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(hash) = node.as_keyword_hash_node() else { return };
        let Some(first) = hash.elements().first() else { return };
        let Some(splat) = first.as_assoc_splat_node() else { return };
        let Some(value) = splat.value() else { return };
        let Some(merge_call) = value.as_call_node() else { return };
        if merge_call.name().as_slice() != b"merge" || merge_call.is_safe_navigation() {
            return;
        }

        let ancestors = ctx.ancestors();
        let len = ancestors.len();
        if len < 2
            || ancestors[len - 1].kind != NodeKind::ArgumentsNode
            || ancestors[len - 2].kind != NodeKind::CallNode
        {
            return;
        }

        if merge_call.block().is_some() {
            return;
        }
        let Some(receiver) = merge_call.receiver() else { return };
        let other_args: Vec<Node<'_>> =
            merge_call.arguments().map_or_else(Vec::new, |a| a.arguments().into_iter().collect());

        let hash_source = String::from_utf8_lossy(ctx.text(receiver.span())).into_owned();
        let other_replacement: Vec<String> = other_args
            .iter()
            .map(|arg| {
                if let Some(literal) = arg.as_hash_node() {
                    let text = ctx.text(literal.as_node().span());
                    String::from_utf8_lossy(&text[1..text.len() - 1]).into_owned()
                } else if arg.as_keyword_hash_node().is_some() {
                    String::from_utf8_lossy(ctx.text(arg.span())).into_owned()
                } else {
                    format!("**{}", String::from_utf8_lossy(ctx.text(arg.span())))
                }
            })
            .collect();

        let replacement = format!("**{hash_source}, {}", other_replacement.join(", "));

        ctx.report_with_fix(
            &Self::META,
            value.span(),
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(splat.as_node().span(), replacement.into_bytes())],
            },
        );
    }
}
