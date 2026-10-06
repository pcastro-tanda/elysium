//! `Sorbet/ForbidTAnyWithNil`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/forbid_t_any_with_nil.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Use `T.nilable` instead of `T.any(..., NilClass, ...)`.";

/// Forbid usage of T.any(NilClass).
#[derive(Debug, Clone)]
pub struct ForbidTAnyWithNil;

impl Rule for ForbidTAnyWithNil {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ForbidTAnyWithNil",
        department: Department::Sorbet,
        summary: "Forbid usage of T.any(NilClass).",
        explanation: "Detect and autocorrect `T.any(..., NilClass, ...)` to `T.nilable(...)`.\n\n```ruby\n# bad\nT.any(String, NilClass)\nT.any(NilClass, String)\nT.any(NilClass, Symbol, String)\n\n# good\nT.nilable(String)\nT.nilable(String)\nT.nilable(T.any(Symbol, String))\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        // `(send (const nil? :T) :any $...)`; `on_csend` cannot match a `send` pattern.
        if call.name().as_slice() != b"any"
            || call.is_safe_navigation()
            || !call.receiver().is_some_and(|receiver| is_const_named(&receiver, b"T"))
        {
            return;
        }
        let mut args: Vec<Node<'_>> =
            call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
        // A `&blk` argument is a `block_pass` child of the `send` in whitequark.
        if let Some(block) = call.block().filter(|block| block.as_block_argument_node().is_some()) {
            args.push(block);
        }

        let (nil_args, non_nil_args): (Vec<&Node<'_>>, Vec<&Node<'_>>) =
            args.iter().partition(|arg: &&Node<'_>| is_const_named(arg, b"NilClass"));
        if nil_args.is_empty() || non_nil_args.is_empty() {
            return;
        }

        let sources: Vec<String> = non_nil_args
            .iter()
            .map(|arg: &&Node<'_>| String::from_utf8_lossy(ctx.text(arg.span())).into_owned())
            .collect();
        let inner = if sources.len() == 1 {
            sources[0].clone()
        } else {
            format!("T.any({})", sources.join(", "))
        };
        let replacement = format!("T.nilable({inner})");

        let span = if call.block().is_some_and(|block| block.as_block_node().is_some()) {
            call_span_excluding_block(&call)
        } else {
            node.span()
        };
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement.into_bytes())],
            },
        );
    }
}

/// `(const nil? :name)`: a bare, non-`::`-prefixed constant.
fn is_const_named(node: &Node<'_>, name: &[u8]) -> bool {
    node.as_constant_read_node().is_some_and(|constant| constant.name().as_slice() == name)
}
