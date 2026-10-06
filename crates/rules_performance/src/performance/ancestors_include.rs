//! `Performance/AncestorsInclude`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/ancestors_include.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `<=` instead of `ancestors.include?`.";

/// Use `A <= B` instead of `A.ancestors.include?(B)`.
#[derive(Debug, Clone)]
pub struct AncestorsInclude;

impl Rule for AncestorsInclude {
    const META: RuleMeta = RuleMeta {
        name: "Performance/AncestorsInclude",
        department: Department::Performance,
        summary: "Use `A <= B` instead of `A.ancestors.include?(B)`.",
        explanation: "\
Identifies usages of `ancestors.include?` and change them to use `<=` instead.

This cop is unsafe because it can't tell whether the receiver is a class or an object.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
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
        if call.is_safe_navigation() || call.name().as_slice() != b"include?" {
            return;
        }
        let Some(inner_node) = call.receiver() else { return };
        let Some(inner) = inner_node.as_call_node() else { return };
        if inner.is_safe_navigation()
            || inner.name().as_slice() != b"ancestors"
            || inner.arguments().is_some()
            || inner.block().is_some()
        {
            return;
        }
        // Exactly one argument (a `&blk` counts as one in whitequark).
        let superclass_span = match (call.arguments(), call.block()) {
            (Some(args), None) => {
                let mut it = args.arguments().iter();
                match (it.next(), it.next()) {
                    (Some(a), None) => a.span(),
                    _ => return,
                }
            }
            _ => return,
        };
        let subclass_source: Vec<u8> = match inner.receiver() {
            Some(subclass) => {
                if !matches!(
                    subclass.kind(),
                    NodeKind::ConstantReadNode | NodeKind::ConstantPathNode
                ) {
                    return;
                }
                ctx.text(subclass.span()).to_vec()
            }
            None => b"self".to_vec(),
        };
        let (Some(ancestors_loc), Some(include_loc)) = (inner.message_loc(), call.message_loc())
        else {
            return;
        };
        let range = Span::new(ancestors_loc.span().start, include_loc.span().end);
        let mut replacement = subclass_source;
        replacement.extend_from_slice(b" <= ");
        replacement.extend_from_slice(ctx.text(superclass_span));
        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(node.span(), replacement)],
        };
        ctx.report_with_fix(&Self::META, range, MSG, fix);
    }
}
