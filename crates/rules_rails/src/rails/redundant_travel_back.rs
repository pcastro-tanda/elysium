//! `Rails/RedundantTravelBack`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/redundant_travel_back.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Redundant `travel_back` detected.";

/// `minimum_target_rails_version 5.2`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 5.2;

/// Checks for redundant `travel_back` calls. Since Rails 5.2, `travel_back`
/// is automatically called at the end of the test.
#[derive(Debug, Clone)]
pub struct RedundantTravelBack {
    /// Whether the target Rails version reaches
    /// `minimum_target_rails_version`.
    supported: bool,
    /// Spans of the `def teardown` methods and `after` blocks entered so far.
    scopes: Vec<Span>,
}

impl Rule for RedundantTravelBack {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RedundantTravelBack",
        department: Department::Rails,
        summary: "Checks for redundant `travel_back` calls.",
        explanation: "Checks for redundant `travel_back` calls.\nSince Rails 5.2, `travel_back` \
                      is automatically called at the end of the test.\n\n```ruby\n# bad\ndef \
                      teardown\n  do_something\n  travel_back\nend\n\n# good\ndef teardown\n  \
                      do_something\nend\n\n# bad\nafter do\n  do_something\n  travel_back\n\
                      end\n\n# good\nafter do\n  do_something\nend\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            supported: options.target_rails_version() >= MINIMUM_TARGET_RAILS_VERSION,
            scopes: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        if let Some(def) = node.as_def_node() {
            // `ancestor.def_type?` excludes `defs`.
            if def.receiver().is_none() && def.name().as_slice() == b"teardown" {
                self.scopes.push(node.span());
            }
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() == b"after" {
            if let Some(block) = call.block() {
                // numbered-parameter and `it` blocks are not `block` nodes.
                let literal = block.as_block_node();
                if literal.is_some_and(|literal| {
                    !literal.parameters().is_some_and(|parameters| {
                        parameters.as_numbered_parameters_node().is_some()
                            || parameters.as_it_parameters_node().is_some()
                    })
                }) {
                    self.scopes.push(block.location().span());
                }
            }
        }
        if call.is_safe_navigation() || call.name().as_slice() != b"travel_back" {
            return;
        }
        let span = node.span();
        // The block node of `after do ... end` does not contain its own send.
        if !self.scopes.iter().any(|scope| scope.start <= span.start && span.end <= scope.end) {
            return;
        }
        let call_span = ruby_ast::ext::call_span_excluding_block(&call);
        ctx.report_with_fix(
            &Self::META,
            call_span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::delete(ctx.whole_lines(call_span))],
            },
        );
    }
}
