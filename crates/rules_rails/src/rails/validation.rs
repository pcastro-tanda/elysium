//! `Rails/Validation`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/validation.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, NodeExt as _};
use ruby_source::Span;
use ruby_ast::{Node, NodeKind};

const TYPES: [&str; 12] = [
    "acceptance",
    "comparison",
    "confirmation",
    "exclusion",
    "format",
    "inclusion",
    "length",
    "numericality",
    "presence",
    "absence",
    "size",
    "uniqueness",
];

/// Checks for the use of old-style attribute validation macros.
#[derive(Debug, Clone)]
pub struct Validation;

impl Rule for Validation {
    const META: RuleMeta = RuleMeta {
        name: "Rails/Validation",
        department: Department::Rails,
        summary: "Use validates :attribute, hash of validations.",
        explanation: "Checks for the use of old-style attribute validation macros.\n\n```ruby\n\
                      # bad\nvalidates_presence_of :foo\n\n# good\nvalidates :foo, presence: true\n```",
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
        let name = call.name();
        let Some(ty) = validation_type(name.as_slice()) else { return };
        if call.receiver().is_some() {
            return;
        }
        let block_arg = call.block().filter(|b| b.as_block_argument_node().is_some());
        let last_argument = call
            .arguments()
            .and_then(|args| args.arguments().iter().last())
            .or(block_arg);
        let Some(last_argument) = last_argument else { return };
        let Some(selector) = call.message_loc() else { return };
        let range = selector.span();

        let current = String::from_utf8_lossy(name.as_slice()).into_owned();
        let message = format!(
            "Prefer the new style validations `validates :column, {ty}: value` over `{current}`."
        );

        let frozen = frozen_array_argument(&last_argument);
        if !is_literal(&last_argument) && !matches!(last_argument.kind(), NodeKind::SplatNode) && !frozen
        {
            // Upstream `return`s from inside the `add_offense` block, which
            // discards the offense too.
            return;
        }

        let new_ty = if ty == "size" { "length" } else { ty };
        let mut edits = vec![Edit::replace(range, b"validates".to_vec())];
        edits.push(correction(ctx, &last_argument, new_ty));
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

fn validation_type(method: &[u8]) -> Option<&'static str> {
    let rest = method.strip_prefix(b"validates_")?.strip_suffix(b"_of")?;
    TYPES.into_iter().find(|t| t.as_bytes() == rest)
}

fn frozen_array_argument(argument: &Node<'_>) -> bool {
    argument
        .as_call_node()
        .is_some_and(|c| c.name().as_slice() == b"freeze" && c.receiver().is_some())
}

fn is_literal(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::ArrayNode
            | NodeKind::HashNode
            | NodeKind::KeywordHashNode
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

fn correction(ctx: &Context<'_>, last: &Node<'_>, ty: &str) -> Edit {
    let span = last.span();
    if last.as_hash_node().is_some() {
        let src = String::from_utf8_lossy(ctx.text(span)).into_owned();
        return Edit::replace(span, format!("{ty}: {src}").into_bytes());
    }
    if last.as_keyword_hash_node().is_some() {
        let src = String::from_utf8_lossy(ctx.text(span)).into_owned();
        return Edit::replace(span, format!("{ty}: {{ {src} }}").into_bytes());
    }
    if last.as_array_node().is_some() {
        return array_edit(ctx, last, span, ty);
    }
    if let Some(call) = last.as_call_node() {
        if call.name().as_slice() == b"freeze" {
            if let Some(receiver) = call.receiver() {
                return array_edit(ctx, &receiver, span, ty);
            }
        }
    }
    Edit::insert(span.end, format!(", {ty}: true").into_bytes())
}

fn array_edit(ctx: &Context<'_>, array: &Node<'_>, replace: Span, ty: &str) -> Edit {
    let mut attributes = Vec::new();
    if let Some(arr) = array.as_array_node() {
        let percent = ctx.text(array.span()).starts_with(b"%");
        for child in &arr.elements() {
            let src = String::from_utf8_lossy(ctx.text(child.span())).into_owned();
            attributes.push(if percent { format!(":{src}") } else { src });
        }
    }
    let joined = attributes.join(", ");
    Edit::replace(replace, format!("{joined}, {ty}: true").into_bytes())
}
