//! `Lint/RedundantRequireStatement`, ported from RuboCop's
//! `lib/rubocop/cop/lint/redundant_require_statement.rb`.
//!
//! Upstream detects the modifier `if`/`unless`/`while`/`until` form via
//! `node.parent.respond_to?(:modifier_form?) && node.parent.modifier_form?`
//! (`loc.end.nil?` in whitequark). Prism has no such flag on these nodes, but
//! a modifier-form conditional always starts at the same offset as its body
//! statement (the keyword comes *after* the body), whereas a block-form one
//! starts at its own keyword, well before the body; comparing the parent's
//! and the `require` call's span starts reproduces the same distinction
//! without re-deriving `end_keyword_loc`/`closing_loc` per node kind.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Side;

/// RuboCop's `MSG`.
const MSG: &str = "Remove unnecessary `require` statement.";

/// Features already loaded by the target Ruby version, per RuboCop's
/// `RUBY_22_LOADED_FEATURES` table and `redundant_feature?`.
fn redundant_feature(feature_name: &[u8], target_ruby_version: f32) -> bool {
    feature_name == b"enumerator"
        || (target_ruby_version >= 2.1 && feature_name == b"thread")
        || (target_ruby_version >= 2.2
            && (feature_name == b"rational" || feature_name == b"complex"))
        || (target_ruby_version >= 2.7 && feature_name == b"ruby2_keywords")
        || (target_ruby_version >= 3.1 && feature_name == b"fiber")
        || (target_ruby_version >= 3.2 && feature_name == b"set")
        || (target_ruby_version >= 4.0 && feature_name == b"pathname")
}

/// Checks for unnecessary `require` statement.
#[derive(Debug, Clone)]
pub struct RedundantRequireStatement {
    target_ruby_version: f32,
}

impl Rule for RedundantRequireStatement {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RedundantRequireStatement",
        department: Department::Lint,
        summary: "Checks for unnecessary `require` statement.",
        explanation: "\
The following features are unnecessary `require` statement because \
they are already loaded. e.g. Ruby 2.2:

```
ruby -ve 'p $LOADED_FEATURES.reject { |feature| %r|/| =~ feature }'
ruby 2.2.8p477 (2017-09-14 revision 59906) [x86_64-darwin13]
[\"enumerator.so\", \"rational.so\", \"complex.so\", \"thread.rb\"]
```

Below are the features that each `TargetRubyVersion` targets.

* 2.0+ ... `enumerator`
* 2.1+ ... `thread`
* 2.2+ ... Add `rational` and `complex` above
* 2.7+ ... Add `ruby2_keywords` above
* 3.1+ ... Add `fiber` above
* 3.2+ ... Add `set` above
* 4.0+ ... Add `pathname` above

This cop target those features.

```ruby
# bad
require 'unloaded_feature'
require 'thread'

# good
require 'unloaded_feature'
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() || call.receiver().is_some() {
            return;
        }
        if call.name().as_slice() != b"require" {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let args = arguments.arguments();
        if args.len() != 1 {
            return;
        }
        let Some(arg) = args.first() else { return };
        let Some(str_node) = arg.as_string_node() else { return };
        if !redundant_feature(str_node.unescaped(), self.target_ruby_version) {
            return;
        }

        let span = node.span();
        let fix = modifier_form_ancestor(ctx.ancestors(), span).map_or_else(
            || Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::delete(ctx.whole_lines(span))],
            },
            |modifier| Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::insert(modifier.span.end, *b"\nend"),
                    Edit::delete(ctx.with_surrounding_space(span, Side::Right, false, false)),
                ],
            },
        );
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}

/// Whether the `require` call at `call_span` is the sole body statement of
/// a modifier-form `if`/`unless`/`while`/`until`, i.e. its immediate parent
/// is the lone-statement `StatementsNode` Prism always wraps bodies in (see
/// the module doc), and that `StatementsNode`'s own parent is a
/// conditional/loop node starting at the same offset as the call (the
/// keyword comes *after* the body only in modifier form).
fn modifier_form_ancestor(
    ancestors: &[linter::NodeInfo],
    call_span: ruby_source::Span,
) -> Option<linter::NodeInfo> {
    let [.., grandparent, parent] = ancestors else { return None };
    if parent.kind != NodeKind::StatementsNode {
        return None;
    }
    let is_conditional = matches!(
        grandparent.kind,
        NodeKind::IfNode | NodeKind::UnlessNode | NodeKind::WhileNode | NodeKind::UntilNode
    );
    (is_conditional && grandparent.span.start == call_span.start).then_some(*grandparent)
}
