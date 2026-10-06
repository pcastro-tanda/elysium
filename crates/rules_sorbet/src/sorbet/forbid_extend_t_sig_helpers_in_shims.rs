//! `Sorbet/ForbidExtendTSigHelpersInShims`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/forbid_extend_t_sig_helpers_in_shims.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeKind};

const MSG: &str = "Extending T::Sig or T::Helpers in a shim is unnecessary";

/// Forbid the use of `extend T::Sig` and `extend T::Helpers` in RBI shims
#[derive(Debug, Clone)]
pub struct ForbidExtendTSigHelpersInShims;

impl Rule for ForbidExtendTSigHelpersInShims {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ForbidExtendTSigHelpersInShims",
        department: Department::Sorbet,
        summary: "Forbid the use of `extend T::Sig` and `extend T::Helpers` in RBI shims",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        // `(send nil? :extend (const (const nil? :T) {:Sig :Helpers}))`
        if call.receiver().is_some()
            || call.is_safe_navigation()
            || call.name().as_slice() != b"extend"
            || call.block().is_some_and(|block| block.as_block_argument_node().is_some())
        {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let list = arguments.arguments();
        let mut iter = list.iter();
        let (Some(arg), None) = (iter.next(), iter.next()) else { return };
        let Some(path) = arg.as_constant_path_node() else { return };
        let Some(parent) = path.parent() else { return };
        if parent.as_constant_read_node().is_none_or(|t| t.name().as_slice() != b"T")
            || !path.name().is_some_and(|n| matches!(n.as_slice(), b"Sig" | b"Helpers"))
        {
            return;
        }
        let span = ruby_ast::ext::call_span_excluding_block(&call);
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::delete(ctx.whole_lines(span))],
        };
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}
