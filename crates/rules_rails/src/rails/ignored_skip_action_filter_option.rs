//! `Rails/IgnoredSkipActionFilterOption`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/ignored_skip_action_filter_option.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

const FILTERS: [&[u8]; 4] =
    [b"skip_after_action", b"skip_around_action", b"skip_before_action", b"skip_action_callback"];

/// Checks that `if` and `only` (or `except`) are not used together as options of `skip_*` action filter.
#[derive(Debug, Clone)]
pub struct IgnoredSkipActionFilterOption;

impl Rule for IgnoredSkipActionFilterOption {
    const META: RuleMeta = RuleMeta {
        name: "Rails/IgnoredSkipActionFilterOption",
        department: Department::Rails,
        summary: "Checks that `if` and `only` (or `except`) are not used together as options of `skip_*` action filter.",
        explanation: "Checks that `if` and `only` (or `except`) are not used together as options \
                      of `skip_*` action filter.\n\nThe `if` option will be ignored when `if` and \
                      `only` are used together. Similarly, the `except` option will be ignored \
                      when `if` and `except` are used together.\n\n```ruby\n# bad\nclass \
                      MyPageController < ApplicationController\n  skip_before_action \
                      :login_required,\n    only: :show, if: :trusted_origin?\nend\n\n# \
                      good\nclass MyPageController < ApplicationController\n  skip_before_action \
                      :login_required,\n    if: -> { trusted_origin? && action_name == \"show\" \
                      }\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.receiver().is_some() || !FILTERS.contains(&call.name().as_slice()) {
            return;
        }
        // A `&block` argument is the last `send` argument in whitequark.
        if call.block().is_some_and(|block| block.as_block_argument_node().is_some()) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let Some(last) = arguments.arguments().iter().last() else { return };
        let elements = match last.kind() {
            NodeKind::HashNode => last.as_hash_node().expect("checked").elements(),
            NodeKind::KeywordHashNode => last.as_keyword_hash_node().expect("checked").elements(),
            _ => return,
        };

        // `options_hash`: symbol-keyed pairs, the last of any duplicated key winning.
        let (mut if_span, mut only, mut except) = (None, false, None);
        for pair in elements.iter().filter_map(|element| element.as_assoc_node()) {
            let Some(key) = pair.key().as_symbol_node() else { continue };
            let span = pair.location().span();
            match key.unescaped() {
                b"if" => if_span = Some(span),
                b"only" => only = true,
                b"except" => except = Some(span),
                _ => {}
            }
        }

        if let (Some(span), true) = (if_span, only) {
            register(ctx, span, "only", "if");
        } else if let (Some(_), Some(span)) = (if_span, except) {
            register(ctx, span, "if", "except");
        }
    }
}

fn register(ctx: &mut Context<'_>, span: Span, prefer: &str, ignore: &str) {
    let message = format!(
        "`{ignore}` option will be ignored when `{prefer}` and `{ignore}` are used together."
    );
    // `remove_node_with_left_space_and_comma`.
    let spaced = ctx.with_surrounding_space(span, Side::Left, true, false);
    let start = if spaced.start > 0 && ctx.source().bytes()[spaced.start as usize - 1] == b',' {
        spaced.start - 1
    } else {
        spaced.start
    };
    ctx.report_with_fix(
        &IgnoredSkipActionFilterOption::META,
        span,
        message,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(Span::new(start, spaced.end), Vec::new())],
        },
    );
}
