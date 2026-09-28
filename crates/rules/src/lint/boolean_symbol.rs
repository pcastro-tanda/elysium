//! `Lint/BooleanSymbol`, ported from RuboCop's
//! `lib/rubocop/cop/lint/boolean_symbol.rb`.
//!
//! Upstream tells apart a plain `:true`/`:false` literal from a hash-label
//! key (`true: 'Foo'`) by checking `parent&.pair_type? && parent.colon?`
//! (whitequark gives the label `sym` node's own `loc.expression` just the
//! bare identifier, folding the trailing `:` into the pair's separate
//! operator location). Prism instead extends a label `SymbolNode`'s own
//! location -- and its `opening_loc`/`closing_loc` shape -- through that
//! trailing colon: a label has no `opening_loc` (no leading `:`) but a
//! `closing_loc` covering the `:`, while an ordinary symbol has an
//! `opening_loc` (the leading `:`, or a quote/`%s{`) and no `closing_loc`
//! (unless quoted). That is enough on its own, with no need to inspect the
//! enclosing `AssocNode`: the offense range for a label uses `value_loc`
//! (the bare identifier, matching whitequark's narrower range) and its
//! autocorrection replaces the *whole* node -- identifier plus trailing
//! `:` -- with `"#{boolean} =>"` in one edit, which is exactly equivalent
//! to upstream's two-step "remove the pair's operator, then replace the
//! key" dance.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Symbol with a boolean name - you probably meant to use `%s`.";

/// Checks for `:true` and `:false` symbols.
#[derive(Debug, Clone)]
pub struct BooleanSymbol;

impl Rule for BooleanSymbol {
    const META: RuleMeta = RuleMeta {
        name: "Lint/BooleanSymbol",
        department: Department::Lint,
        summary: "Checks for `:true` and `:false` symbols.",
        explanation: "In most cases it would be a typo.",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::SymbolNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let sym = node.as_symbol_node().expect("kind matched");
        let boolean = sym.unescaped();
        if boolean != b"true" && boolean != b"false" {
            return;
        }
        if in_percent_symbol_array(ctx) {
            return;
        }

        // A label key (`true: 'Foo'`) has no leading `:` of its own but a
        // `closing_loc` covering the trailing `:`; an ordinary symbol (bare,
        // `:`-prefixed, quoted, or `=>`-keyed) never has both absent-opening
        // and present-closing at once.
        let is_label = sym.opening_loc().is_none() && sym.closing_loc().is_some();
        let report_span =
            if is_label { sym.value_loc().expect("label has a value").span() } else { node.span() };

        let boolean = String::from_utf8_lossy(boolean).into_owned();
        let message = MSG.replace("%s", &boolean);
        let replacement = if is_label { format!("{boolean} =>") } else { boolean };
        ctx.report_with_fix(
            &Self::META,
            report_span,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(node.span(), replacement.into_bytes())],
            },
        );
    }
}

/// RuboCop's `parent&.array_type? && parent.percent_literal?(:symbol)`:
/// an array literal opened with `%i(`/`%I(`. `Context` only exposes the
/// parent's kind and span (not the node itself), but the opening delimiter
/// lives at the very start of that span, so reading it straight off the
/// source is enough.
fn in_percent_symbol_array(ctx: &Context<'_>) -> bool {
    let Some(parent) = ctx.parent() else { return false };
    if parent.kind != NodeKind::ArrayNode {
        return false;
    }
    let text = ctx.text(parent.span);
    text.starts_with(b"%i") || text.starts_with(b"%I")
}
