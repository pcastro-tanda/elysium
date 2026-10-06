//! `Rails/DelegateAllowBlank`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/delegate_allow_blank.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "`allow_blank` is not a valid option, use `allow_nil`.";

/// Looks for delegations that pass `:allow_blank` as an option instead of
/// `:allow_nil`.
#[derive(Debug, Clone)]
pub struct DelegateAllowBlank;

impl Rule for DelegateAllowBlank {
    const META: RuleMeta = RuleMeta {
        name: "Rails/DelegateAllowBlank",
        department: Department::Rails,
        summary: "Do not use allow_blank as an option to delegate.",
        explanation: "Looks for delegations that pass :allow_blank as an option instead of \
                      :allow_nil. :allow_blank is not a valid option to pass to \
                      ActiveSupport#delegate.\n\n```ruby\n# bad\ndelegate :foo, to: :bar, \
                      allow_blank: true\n\n# good\ndelegate :foo, to: :bar, allow_nil: true\n```",
        enabled_by_default: true,
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
        // `(send nil? :delegate _ (hash <$(pair (sym :allow_blank) true) ...>))`.
        let Some(call) = node.as_call_node() else { return };
        if call.receiver().is_some()
            || call.is_safe_navigation()
            || call.name().as_slice() != b"delegate"
        {
            return;
        }
        if call.block().is_some_and(|block| block.as_block_argument_node().is_some()) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let mut arguments = arguments.arguments().iter();
        let (Some(_), Some(options), None) = (arguments.next(), arguments.next(), arguments.next())
        else {
            return;
        };
        let elements: Vec<Node<'_>> = if let Some(hash) = options.as_keyword_hash_node() {
            hash.elements().iter().collect()
        } else if let Some(hash) = options.as_hash_node() {
            hash.elements().iter().collect()
        } else {
            return;
        };
        let Some(key) = elements.iter().find_map(allow_blank_true_key) else { return };
        let pair_span = key.pair;
        ctx.report_with_fix(
            &Self::META,
            pair_span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(key.key, b"allow_nil".to_vec())],
            },
        );
    }
}

struct AllowBlankPair {
    pair: Span,
    /// The key as whitequark's `sym` expression: no trailing `:` of a label.
    key: Span,
}

/// `(pair (sym :allow_blank) true)`.
fn allow_blank_true_key(element: &Node<'_>) -> Option<AllowBlankPair> {
    let assoc = element.as_assoc_node()?;
    assoc.value().as_true_node()?;
    let key = assoc.key();
    let symbol = key.as_symbol_node()?;
    if symbol.unescaped() != b"allow_blank" {
        return None;
    }
    let mut span = symbol.location().span();
    if symbol.closing_loc().is_some_and(|closing| closing.as_slice().ends_with(b":")) {
        span = Span::from_usize(span.start as usize, span.end as usize - 1);
    }
    Some(AllowBlankPair { pair: element.span(), key: span })
}
