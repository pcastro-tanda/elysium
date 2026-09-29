//! `Style/BarePercentLiterals`, ported from RuboCop's
//! `lib/rubocop/cop/style/bare_percent_literals.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// RuboCop's `ConfigurableEnforcedStyle` `style` for this cop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// Prefer `%()` over `%Q()`.
    BarePercent,
    /// Prefer `%Q()` over bare `%()`/`%||`/etc.
    PercentQ,
}

/// Checks if usage of `%()` or `%Q()` matches configuration.
#[derive(Debug, Clone)]
pub struct BarePercentLiterals {
    style: Style,
}

impl Rule for BarePercentLiterals {
    const META: RuleMeta = RuleMeta {
        name: "Style/BarePercentLiterals",
        department: Department::Style,
        summary: "Checks if usage of %() or %Q() matches configuration.",
        explanation: "Checks if usage of `%()` or `%Q()` matches configuration. Consistent use \
                      of one style makes the codebase easier to read.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StringNode, NodeKind::InterpolatedStringNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("bare_percent"),
            allowed: &["percent_q", "bare_percent"],
            doc: "The preferred style for bare `%()` vs. `%Q()` literals.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "percent_q" => Style::PercentQ,
            _ => Style::BarePercent,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if ruby_ast::ext::is_heredoc(node) {
            return;
        }
        let opening = match node {
            Node::StringNode { .. } => node.as_string_node().expect("kind matched").opening_loc(),
            Node::InterpolatedStringNode { .. } => {
                node.as_interpolated_string_node().expect("kind matched").opening_loc()
            }
            _ => return,
        };
        let Some(opening) = opening else { return };
        let source = ctx.text(opening.span());

        // RuboCop's `requires_percent_q?`: `/^%[^\w]/.match?(source)` --
        // a bare `%` literal whose delimiter is not a word character (so
        // not `%q`/`%Q`/plain `"`/`'`).
        let (good, bad) = if self.style == Style::PercentQ
            && source.first() == Some(&b'%')
            && source.get(1).is_some_and(|&b| !b.is_ascii_alphanumeric() && b != b'_')
        {
            ("Q", "")
        } else if self.style == Style::BarePercent && source.starts_with(b"%Q") {
            ("", "Q")
        } else {
            return;
        };

        let msg = format!("Use `%{good}` instead of `%{bad}`.");
        // RuboCop's `corrected`: `source.start_with?('%Q') ? '%' :
        // '%Q'`, substituted for the leading `/%Q?/` match (i.e. bare `%`
        // or `%Q`).
        let (replacement, prefix_len): (&[u8], usize) =
            if source.starts_with(b"%Q") { (b"%", 2) } else { (b"%Q", 1) };
        let corrected = [replacement, &source[prefix_len..]].concat();

        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(opening.span(), corrected)],
        };
        ctx.report_with_fix(&Self::META, opening.span(), msg, fix);
    }
}
