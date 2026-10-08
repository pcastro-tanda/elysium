//! `Style/RedundantFilterChain`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_filter_chain.rb`.
//!
//! Upstream's `select_predicate?` node matcher distinguishes a `select`
//! (etc.) call wrapped in an `any_block` (a real block, any parameter
//! style) from one whose sole argument is a `block_pass` (`&:odd?`); Prism
//! unifies both shapes behind the same `CallNode::block()` field (a
//! `BlockNode` for the former, a `BlockArgumentNode` for the latter), so
//! this port only needs `receiver.block().is_some()` to cover both --
//! matching the registered examples' numbered (`_1`), `it`, and ordinary
//! `|x|` parameters identically, since none of those change which node
//! kind `block()` returns.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `RESTRICT_ON_SEND` (`%i[any? empty? none? one?] + RAILS_METHODS`).
fn is_restrict(name: &[u8]) -> bool {
    matches!(name, b"any?" | b"empty?" | b"none?" | b"one?" | b"many?" | b"present?")
}

/// RuboCop's `RAILS_METHODS`.
fn is_rails_method(name: &[u8]) -> bool {
    matches!(name, b"many?" | b"present?")
}

/// `select`/`filter`/`find_all`.
fn is_filter_method(name: &[u8]) -> bool {
    matches!(name, b"select" | b"filter" | b"find_all")
}

/// RuboCop's `REPLACEMENT_METHODS`.
fn replacement_for(name: &[u8]) -> &'static [u8] {
    match name {
        b"any?" | b"present?" => b"any?",
        b"empty?" | b"none?" => b"none?",
        b"one?" => b"one?",
        b"many?" => b"many?",
        _ => unreachable!("gated by is_restrict"),
    }
}

/// Identifies usages of `any?`, `empty?`, `none?` or `one?` predicate methods chained to `select`/`filter`/`find_all` and change them to use predicate method instead.
#[derive(Debug, Clone)]
pub struct RedundantFilterChain {
    active_support_extensions_enabled: bool,
}

impl Rule for RedundantFilterChain {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantFilterChain",
        department: Department::Style,
        summary: "Identifies usages of `any?`, `empty?`, `none?` or `one?` predicate methods chained to `select`/`filter`/`find_all` and change them to use predicate method instead.",
        explanation: "\
Identifies usages of `any?`, `empty?` or `none?` predicate methods
chained to `select`/`filter`/`find_all` and change them to use predicate method instead.

```ruby
# bad
arr.select { |x| x > 1 }.any?

# good
arr.any? { |x| x > 1 }

# bad
arr.select { |x| x > 1 }.empty?
arr.select { |x| x > 1 }.none?

# good
arr.none? { |x| x > 1 }

# good
relation.select(:name).any?
arr.select { |x| x > 1 }.any?(&:odd?)
```

With `AllCops: ActiveSupportExtensionsEnabled: true`, `many?` and
`present?` are treated the same way (`many?`/`any?` respectively).",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
`array.select.any?` evaluates every element through `select`'s own
enumeration, while `array.any?` short-circuits on the first match; the
autocorrect is marked unsafe (matching upstream's `SafeAutoCorrect: false`)
but this port does not otherwise special-case side-effecting blocks.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let enabled = options
            .peer("AllCops", "ActiveSupportExtensionsEnabled")
            .and_then(linter::OptionValue::as_bool)
            .unwrap_or(false);
        Ok(Self { active_support_extensions_enabled: enabled })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(predicate) = node.as_call_node() else { return };
        let name = predicate.name().as_slice();
        if !is_restrict(name) {
            return;
        }
        if predicate.arguments().is_some() || predicate.block().is_some() {
            return;
        }

        let Some(receiver) = predicate.receiver() else { return };
        let Some(select_call) = receiver.as_call_node() else { return };
        let select_name = select_call.name().as_slice();
        if !is_filter_method(select_name) {
            return;
        }
        if select_call.block().is_none() {
            return;
        }

        if is_rails_method(name) && !self.active_support_extensions_enabled {
            return;
        }

        let Some(select_selector) = select_call.message_loc().map(|l| l.span()) else { return };
        let Some(predicate_selector) = predicate.message_loc().map(|l| l.span()) else { return };

        let replacement = replacement_for(name);
        let offense_range = select_selector.join(predicate_selector);
        let message = format!(
            "Use `{}` instead of `{}.{}`.",
            String::from_utf8_lossy(replacement),
            String::from_utf8_lossy(select_name),
            String::from_utf8_lossy(name),
        );

        let predicate_range = Span::new(receiver.span().end, predicate_selector.end);
        let edits = vec![
            Edit::delete(predicate_range),
            Edit::replace(select_selector, replacement.to_vec()),
        ];

        ctx.report_with_fix(
            &Self::META,
            offense_range,
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}
