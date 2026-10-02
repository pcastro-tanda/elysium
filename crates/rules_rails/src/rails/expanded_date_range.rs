//! `Rails/ExpandedDateRange`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/expanded_date_range.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

use super::rails_version::target_rails_version;

/// `minimum_target_rails_version 5.1`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 5.1;

/// `PREFERRED_METHODS` and `MAPPED_DATE_RANGE_METHODS`:
/// `(beginning method, preferred method, end method)`.
const METHODS: [(&[u8], &str, &[u8]); 5] = [
    (b"beginning_of_day", "all_day", b"end_of_day"),
    (b"beginning_of_week", "all_week", b"end_of_week"),
    (b"beginning_of_month", "all_month", b"end_of_month"),
    (b"beginning_of_quarter", "all_quarter", b"end_of_quarter"),
    (b"beginning_of_year", "all_year", b"end_of_year"),
];

/// Checks for expanded date range.
#[derive(Debug, Clone)]
pub struct ExpandedDateRange {
    /// Whether the target Rails version reaches `minimum_target_rails_version`.
    supported: bool,
}

/// A whitequark `send` with a receiver: its receiver, method name and
/// arguments (a `&block` argument included).
struct Send<'pr> {
    receiver: Node<'pr>,
    name: Vec<u8>,
    arguments: Vec<Node<'pr>>,
}

impl Rule for ExpandedDateRange {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ExpandedDateRange",
        department: Department::Rails,
        summary: "Checks for expanded date range.",
        explanation: "Checks for expanded date range. It checks `all_day`, `all_week`, \
                      `all_month`, `all_quarter` and `all_year` methods.\n\n```ruby\n# \
                      bad\ncreated_at: date.beginning_of_day..date.end_of_day\ncreated_at: \
                      date.beginning_of_week..date.end_of_week\ncreated_at: \
                      date.beginning_of_month..date.end_of_month\ncreated_at: \
                      date.beginning_of_quarter..date.end_of_quarter\ncreated_at: \
                      date.beginning_of_year..date.end_of_year\n\n# good\ncreated_at: \
                      date.all_day\ncreated_at: date.all_week\ncreated_at: \
                      date.all_month\ncreated_at: date.all_quarter\ncreated_at: \
                      date.all_year\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::RangeNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { supported: target_rails_version(options) >= MINIMUM_TARGET_RAILS_VERSION })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(range) = node.as_range_node() else { return };
        // `on_irange`.
        if range.is_exclude_end() {
            return;
        }
        let (Some(left), Some(right)) = (range.left(), range.right()) else { return };
        let (Some(begin_node), Some(end_node)) = (as_send(&left), as_send(&right)) else {
            return;
        };

        // `allow?`.
        let source = |node: &Node<'_>| ctx.text(node.span());
        if source(&begin_node.receiver) != source(&end_node.receiver) {
            return;
        }
        let Some((_, preferred, _)) = METHODS
            .iter()
            .find(|(begin, _, end)| *begin == begin_node.name && *end == end_node.name)
        else {
            return;
        };

        let mut preferred_method =
            format!("{}.{preferred}", String::from_utf8_lossy(source(&begin_node.receiver)));
        if begin_node.name == b"beginning_of_week"
            && begin_node.arguments.len() == 1
            && end_node.arguments.len() == 1
        {
            let argument = source(&begin_node.arguments[0]);
            if argument != source(&end_node.arguments[0]) {
                return;
            }
            preferred_method.push('(');
            preferred_method.push_str(&String::from_utf8_lossy(argument));
            preferred_method.push(')');
        } else if !begin_node.arguments.is_empty() || !end_node.arguments.is_empty() {
            return;
        }

        let span = node.span();
        ctx.report_with_fix(
            &Self::META,
            span,
            format!("Use `{preferred_method}` instead."),
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, preferred_method.into_bytes())],
            },
        );
    }
}

/// `receiver_source`'s guard: `node.send_type?` (so neither `&.` nor a call
/// with a literal block) with a receiver.
fn as_send<'pr>(node: &Node<'pr>) -> Option<Send<'pr>> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() {
        return None;
    }
    let mut arguments: Vec<Node<'pr>> =
        call.arguments().map_or_else(Vec::new, |args| args.arguments().iter().collect());
    if let Some(block) = call.block() {
        block.as_block_argument_node()?;
        arguments.push(block);
    }
    Some(Send { receiver: call.receiver()?, name: call.name().as_slice().to_vec(), arguments })
}
