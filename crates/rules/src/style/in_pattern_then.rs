//! `Style/InPatternThen`, ported from RuboCop's
//! `lib/rubocop/cop/style/in_pattern_then.rb`.
//!
//! Prism's `InNode` has no location for the `;` separator used instead of
//! `then` (unlike `then_loc`, which is `Some` only for an explicit `then`),
//! so [`semicolon_span`] re-derives it: the first `;` byte between the
//! pattern's end and the body's start, mirroring whitequark's `node.loc.begin`
//! for this shape.
//!
//! whitequark's `match_alt_type?` is Prism's `AlternationPatternNode`;
//! [`alternative_pattern_sources`] mirrors upstream's
//! `collect_alternative_patterns`, recursing into the left operand only
//! while it is itself an alternation.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Checks for `in;` uses in `case` expressions.
#[derive(Debug, Clone)]
pub struct InPatternThen {
    target_ruby_version: f32,
}

impl Rule for InPatternThen {
    const META: RuleMeta = RuleMeta {
        name: "Style/InPatternThen",
        department: Department::Style,
        summary: "Checks for `in;` uses in `case` expressions.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::InNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.target_ruby_version < 2.7 {
            return;
        }
        let Some(in_node) = node.as_in_node() else { return };
        if !ctx.is_single_line(node.span()) || in_node.then_loc().is_some() {
            return;
        }
        let Some(statements) = in_node.statements() else { return };

        let pattern = in_node.pattern();
        let between = Span::new(pattern.span().end, statements.as_node().span().start);
        let Some(offset) = ctx.text(between).iter().position(|&b| b == b';') else { return };
        let offset = u32::try_from(offset).expect("offset exceeds u32");
        let semicolon = Span::new(between.start + offset, between.start + offset + 1);

        let pattern_source = if let Some(alt) = pattern.as_alternation_pattern_node() {
            let sources: Vec<String> = alternative_pattern_sources(ctx, &alt)
                .into_iter()
                .map(|span| String::from_utf8_lossy(ctx.text(span)).into_owned())
                .collect();
            sources.join(" | ")
        } else {
            String::from_utf8_lossy(ctx.text(pattern.span())).into_owned()
        };

        let message =
            format!("Do not use `in {pattern_source};`. Use `in {pattern_source} then` instead.");

        ctx.report_with_fix(
            &Self::META,
            semicolon,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(semicolon, b" then".to_vec())],
            },
        );
    }
}

/// RuboCop's `collect_alternative_patterns`: recurses into the left operand
/// only while it is itself an alternation, then appends each right operand
/// in order.
fn alternative_pattern_sources(
    ctx: &Context<'_>,
    alt: &ruby_ast::node::AlternationPatternNode<'_>,
) -> Vec<Span> {
    let _ = ctx;
    let left = alt.left();
    if let Some(left_alt) = left.as_alternation_pattern_node() {
        let mut sources = alternative_pattern_sources(ctx, &left_alt);
        sources.push(alt.right().span());
        sources
    } else {
        vec![left.span(), alt.right().span()]
    }
}
