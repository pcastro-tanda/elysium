//! `Rails/Pick`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/pick.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `minimum_target_rails_version 6.0`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 6.0;

/// Enforces the use of `pick` over `pluck(...).first`.
#[derive(Debug, Clone)]
pub struct Pick {
    supported: bool,
}

impl Rule for Pick {
    const META: RuleMeta = RuleMeta {
        name: "Rails/Pick",
        department: Department::Rails,
        summary: "Prefer `pick` over `pluck(...).first`.",
        explanation: "Enforces the use of `pick` over `pluck(...).first`.\n\nUsing `pluck` \
                      followed by `first` creates an intermediate array, which `pick` avoids. \
                      When called on an Active Record relation, `pick` adds a limit to the \
                      query so that only one value is fetched from the database.\n\nNote that \
                      when `pick` is added to a relation with an existing limit, it causes a \
                      subquery to be added. In most cases this is undesirable, and care should \
                      be taken while resolving this violation.\n\nThis cop is unsafe because \
                      `pluck` is defined on both `ActiveRecord::Relation` and `Enumerable`, \
                      whereas `pick` is only defined on `ActiveRecord::Relation` in Rails 6.0. \
                      This was addressed in Rails 6.1 via rails/rails#38760, at which point the \
                      cop is safe.\n\n```ruby\n# bad\nModel.pluck(:a).first\n[{ a: :b, c: :d \
                      }].pluck(:a, :b).first\n\n# good\nModel.pick(:a)\n[{ a: :b, c: :d \
                      }].pick(:a, :b)\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { supported: options.target_rails_version() >= MINIMUM_TARGET_RAILS_VERSION })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        // `(call (call _ :pluck ...) :first)`
        if call.name().as_slice() != b"first"
            || call.arguments().is_some()
            || call.block().is_some_and(|b| b.as_block_argument_node().is_some())
        {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(pluck) = receiver.as_call_node() else { return };
        if pluck.name().as_slice() != b"pluck"
            || pluck.block().is_some_and(|b| b.as_block_node().is_some())
        {
            return;
        }
        let (Some(pluck_selector), Some(first_selector)) =
            (pluck.message_loc(), call.message_loc())
        else {
            return;
        };
        let pluck_selector = pluck_selector.span();
        let first_selector = first_selector.span();
        let range = Span::new(pluck_selector.start, first_selector.end);

        let mut args: Vec<String> = pluck.arguments().map_or_else(Vec::new, |a| {
            a.arguments()
                .iter()
                .map(|arg| String::from_utf8_lossy(ctx.text(arg.span())).into_owned())
                .collect()
        });
        if let Some(block_pass) = pluck.block() {
            args.push(String::from_utf8_lossy(ctx.text(block_pass.span())).into_owned());
        }
        let message = format!(
            "Prefer `pick({})` over `{}`.",
            args.join(", "),
            String::from_utf8_lossy(ctx.text(range))
        );
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![
                    Edit::delete(Span::new(receiver.span().end, first_selector.end)),
                    Edit::replace(pluck_selector, b"pick".to_vec()),
                ],
            },
        );
    }
}
