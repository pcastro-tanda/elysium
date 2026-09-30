//! `Lint/RegexpAsCondition`, ported from RuboCop's
//! `lib/rubocop/cop/lint/regexp_as_condition.rb`.
//!
//! Upstream guards against re-flagging its own correction across the fix
//! loop with `ignore_node`/`part_of_ignored_node?`, a byte-range check on
//! state that survives across re-investigations of the same cop instance
//! (see `rubocop/cop/ignored_node.rb`). That state has no elysium
//! equivalent -- rules are stateless and recreated fresh each fix round --
//! but it only matters for the plain (non-`!`) case, where the fix appends
//! `" =~ $_"` right after the regexp literal; [`already_corrected`]
//! recognizes that exact trailing text and skips re-reporting it,
//! converging exactly where upstream's stateful guard does. `!` binds
//! tighter than `=~`, so a bare append after `!<regexp>` would produce
//! `!<regexp> =~ $_`, parsing as `(!<regexp>) =~ $_` -- the regexp itself
//! stays nested under `!` with no `=~` of its own, so it would still parse
//! as a bare `MatchLastLineNode` on the next round. RuboCop 1.91 fixes this
//! at the source: when the immediate parent is a `!` call, the whole `!`
//! call is replaced with `!(<regexp> =~ $_)` instead, so the regexp gets
//! its own explicit `=~` and stops being a `MatchLastLineNode` in one
//! round -- [`already_corrected`] is therefore never reached for that case.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str =
    "Do not use regexp literal as a condition. The regexp literal matches `$_` implicitly.";

/// True when `span` is immediately followed by the exact text this cop's own
/// fix appends (`" =~ $_"`), i.e. this node is already corrected.
fn already_corrected(ctx: &Context<'_>, span: Span) -> bool {
    const SUFFIX: &[u8] = b" =~ $_";
    let bytes = ctx.source().bytes();
    let end = span.end as usize;
    bytes.get(end..end + SUFFIX.len()) == Some(SUFFIX)
}

/// RuboCop's `node.parent&.send_type? && node.parent.method?(:!)`: `node`'s
/// immediate parent is a unary `!` call with `node` as its receiver. A
/// unary-`!` `CallNode`'s own span always starts at the `!` and ends at its
/// receiver's end (there is no room for anything else in between for a
/// bare `!<expr>`), so checking the parent's span ends where `node`'s does
/// and starts with `!` identifies this shape without needing the parent
/// `Node` itself.
fn bang_parent(ctx: &Context<'_>, span: Span) -> Option<Span> {
    let parent = ctx.parent()?;
    if parent.kind != NodeKind::CallNode || parent.span.end != span.end {
        return None;
    }
    (ctx.text(parent.span).first() == Some(&b'!')).then_some(parent.span)
}

/// Do not use regexp literal as a condition. The regexp literal matches `$_` implicitly.
#[derive(Debug, Clone)]
pub struct RegexpAsCondition;

impl Rule for RegexpAsCondition {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RegexpAsCondition",
        department: Department::Lint,
        summary:
            "Do not use regexp literal as a condition. The regexp literal matches `$_` implicitly.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::MatchLastLineNode, NodeKind::InterpolatedMatchLastLineNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        // A regexp literal only parses as `MatchLastLineNode` when it sits
        // directly in an `if`/`unless`/`while`/`until` predicate, or under
        // `!` -- Prism's equivalent of whitequark's `match_current_line`
        // node. Mirror upstream's `ancestors.none?(&:conditional?)` guard,
        // which only actually excludes the bare `!/foo/` (or plain `/foo/`)
        // case: a `MatchLastLineNode` with no enclosing conditional.
        let in_conditional = ctx.ancestors().iter().any(|ancestor| {
            matches!(
                ancestor.kind,
                NodeKind::IfNode | NodeKind::UnlessNode | NodeKind::WhileNode | NodeKind::UntilNode
            )
        });
        if !in_conditional {
            return;
        }

        let span = node.span();
        if already_corrected(ctx, span) {
            return;
        }

        let (fix_span, replacement) = if let Some(bang_span) = bang_parent(ctx, span) {
            let mut replacement = Vec::with_capacity(span.len() as usize + b"!( =~ $_)".len());
            replacement.extend_from_slice(b"!(");
            replacement.extend_from_slice(ctx.text(span));
            replacement.extend_from_slice(b" =~ $_)");
            (bang_span, replacement)
        } else {
            let mut replacement = Vec::with_capacity(span.len() as usize + b" =~ $_".len());
            replacement.extend_from_slice(ctx.text(span));
            replacement.extend_from_slice(b" =~ $_");
            (span, replacement)
        };
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(fix_span, replacement)],
            },
        );
    }
}
