//! `Rails/RedirectBackOrTo`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/redirect_back_or_to.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::AssocNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `redirect_back_or_to` instead of `redirect_back` with `:fallback_location` keyword argument.";

/// `minimum_target_rails_version 7.0`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 7.0;
/// Prefer `redirect_back_or_to` over `redirect_back` with a `:fallback_location` option.
#[derive(Debug, Clone)]
pub struct RedirectBackOrTo {
    /// Whether the target Rails version reaches `minimum_target_rails_version`.
    supported: bool,
}

impl Rule for RedirectBackOrTo {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RedirectBackOrTo",
        department: Department::Rails,
        summary:
            "Use `redirect_back_or_to` instead of `redirect_back` with `fallback_location` option.",
        explanation: "Prefer `redirect_back_or_to` over `redirect_back` with a \
                      `fallback_location` keyword argument (Rails 7.0+).\n\n\
                      ```ruby\n# bad\nredirect_back(fallback_location: root_path)\n\n\
                      # good\nredirect_back_or_to(root_path)\n```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; the locked `railties` version from `Gemfile.lock` is used when no `TargetRailsVersion` is set.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { supported: options.target_rails_version() >= MINIMUM_TARGET_RAILS_VERSION })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"redirect_back"
            || call.receiver().is_some()
            || call.is_safe_navigation()
            || call.block().is_some()
        {
            return;
        }
        // `(send nil? :redirect_back (hash ...))`: a lone hash argument.
        let Some(arguments) = call.arguments() else { return };
        let mut arguments = arguments.arguments().iter();
        let (Some(hash_arg), None) = (arguments.next(), arguments.next()) else { return };
        let elements: Vec<Node<'_>> = if let Some(hash) = hash_arg.as_hash_node() {
            hash.elements().iter().collect()
        } else if let Some(hash) = hash_arg.as_keyword_hash_node() {
            hash.elements().iter().collect()
        } else {
            return;
        };
        let Some(fallback_index) = elements.iter().position(is_fallback_pair) else { return };
        let Some(selector) = call.message_loc() else { return };
        let selector = selector.span();
        let fallback = elements[fallback_index].as_assoc_node().expect("matched a pair");

        let mut edits = vec![Edit::replace(selector, b"redirect_back_or_to".to_vec())];
        let value = ctx.text(fallback.value().span()).to_vec();
        let pairs: Vec<AssocNode<'_>> = elements.iter().filter_map(Node::as_assoc_node).collect();
        if pairs.len() == 1 {
            let mut replacement = value;
            for (index, element) in elements.iter().enumerate() {
                if index != fallback_index {
                    replacement.extend_from_slice(b", ");
                    replacement.extend_from_slice(ctx.text(element.span()));
                }
            }
            edits.push(Edit::replace(hash_arg.span(), replacement));
        } else {
            let fallback_span = fallback.as_node().span();
            let index = pairs
                .iter()
                .position(|pair| pair.as_node().span() == fallback_span)
                .expect("fallback is a pair");
            if index == 0 {
                let next = pairs[1].as_node().span();
                edits.push(Edit::replace(Span::new(fallback_span.start, next.start), Vec::new()));
            } else {
                let previous = pairs[index - 1].as_node().span();
                edits.push(Edit::replace(Span::new(previous.end, fallback_span.end), Vec::new()));
            }
            let first = pairs
                .iter()
                .find(|pair| pair.as_node().span() != fallback_span)
                .expect("more than one pair")
                .as_node()
                .span();
            let mut insertion = value;
            insertion.extend_from_slice(b", ");
            edits.push(Edit::replace(Span::new(first.start, first.start), insertion));
        }
        if call.opening_loc().is_none() {
            // `wrap_with_parentheses`.
            edits
                .push(Edit::replace(Span::new(selector.end, hash_arg.span().start), b"(".to_vec()));
            let end = call.as_node().span().end;
            edits.push(Edit::replace(Span::new(end, end), b")".to_vec()));
        }
        ctx.report_with_fix(
            &Self::META,
            selector,
            MSG,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

/// `(pair (sym :fallback_location) _)`.
fn is_fallback_pair(element: &Node<'_>) -> bool {
    element.as_assoc_node().is_some_and(|pair| {
        pair.key().as_symbol_node().is_some_and(|key| key.unescaped() == b"fallback_location")
    })
}
