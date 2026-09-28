//! `Lint/RescueType`, ported from RuboCop's
//! `lib/rubocop/cop/lint/rescue_type.rb`.
//!
//! RuboCop's whitequark-based `on_resbody(node)` fires once per `resbody`
//! child of a `:rescue` node; Prism gives each `rescue` clause its own
//! `RescueNode` kind directly, so subscribing to `NodeKind::RescueNode`
//! reaches exactly the same clauses one at a time (see `rescue_exception.rs`
//! for the same observation).
//!
//! Upstream's offense range is `node.loc.keyword.join(node.children.first
//! .source_range)`, where `children.first` is the whitequark exception list
//! -- the source range from the first exception through the last, never
//! extending into the `=>` variable or the clause's body. [`exceptions_span`]
//! rebuilds that from the clause's own `keyword_loc`/`exceptions` children.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::RescueNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `INVALID_TYPES`: node kinds that always raise `TypeError` when
/// rescued, because they cannot be coerced to a class/module.
fn is_invalid_exception(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::ArrayNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::FloatNode
            | NodeKind::HashNode
            | NodeKind::NilNode
            | NodeKind::IntegerNode
            | NodeKind::StringNode
            | NodeKind::SymbolNode
    )
}

/// The exception list's own span: from the `rescue` keyword through the end
/// of the last exception expression, never into the `=>` variable or body.
fn exceptions_span(rescue_node: &RescueNode<'_>, last_end: u32) -> Span {
    Span::new(rescue_node.keyword_loc().span().start, last_end)
}

/// Avoid rescuing from non constants that could result in a `TypeError`.
#[derive(Debug, Clone)]
pub struct RescueType;

impl Rule for RescueType {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RescueType",
        department: Department::Lint,
        summary: "Avoid rescuing from non constants that could result in a `TypeError`.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::RescueNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let rescue_node = node.as_rescue_node().expect("kind matched");
        let exceptions = rescue_node.exceptions();
        let Some(last_exception) = exceptions.iter().last() else { return };

        let mut invalid_texts = Vec::new();
        let mut valid_texts = Vec::new();
        for exception in &exceptions {
            let text = String::from_utf8_lossy(ctx.text(exception.span())).into_owned();
            if is_invalid_exception(&exception) {
                invalid_texts.push(text);
            } else {
                valid_texts.push(text);
            }
        }
        if invalid_texts.is_empty() {
            return;
        }

        let offense_span = exceptions_span(&rescue_node, last_exception.span().end);
        let message = format!(
            "Rescuing from `{}` will raise a `TypeError` instead of catching the actual exception.",
            invalid_texts.join(", ")
        );

        let correction = if valid_texts.is_empty() {
            String::new()
        } else {
            format!(" {}", valid_texts.join(", "))
        };
        let fix_span = Span::new(rescue_node.keyword_loc().span().end, last_exception.span().end);

        ctx.report_with_fix(
            &Self::META,
            offense_span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(fix_span, correction.into_bytes())],
            },
        );
    }
}
