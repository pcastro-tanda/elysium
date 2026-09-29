//! `Style/RescueStandardError`, ported from RuboCop's
//! `lib/rubocop/cop/style/rescue_standard_error.rb`.
//!
//! Upstream's `on_resbody` guards every clause with `rescue_modifier?`
//! (whitequark parses `foo rescue nil` as a `:rescue` node too, see
//! `duplicate_rescue_exception.rs`'s module doc). Prism gives the modifier
//! form its own [`NodeKind::RescueModifierNode`] kind, so this rule simply
//! never subscribes to it and the guard has no Prism equivalent to port.
//!
//! `rescue_without_error_class?` (`(resbody nil? _ _)`) is Prism's
//! `RescueNode::exceptions()` being empty: no explicit exception list at
//! all, regardless of whether a `=> e` reference is present. `implicit`
//! style's `rescue_standard_error?` (`(resbody $(array (const {nil? cbase}
//! :StandardError)) _ _)`) requires the exception list to hold exactly one
//! bare-or-toplevel `StandardError` constant and nothing else -- a second
//! exception (`rescue StandardError, SecurityError`) or a namespaced/other
//! constant never matches, same as upstream's node pattern.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::node::RescueNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG_IMPLICIT`.
const MSG_IMPLICIT: &str = "Omit the error class when rescuing `StandardError` by itself.";
/// RuboCop's `MSG_EXPLICIT`.
const MSG_EXPLICIT: &str = "Avoid rescuing without specifying an error class.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Implicit,
    Explicit,
}

/// RuboCop's `rescue_standard_error?`: the clause's exception list is
/// exactly one bare-or-toplevel `StandardError` constant.
fn standard_error_only<'a>(clause: &RescueNode<'a>) -> Option<Node<'a>> {
    let exceptions = clause.exceptions();
    if exceptions.len() != 1 {
        return None;
    }
    let exception = exceptions.first()?;
    if is_bare_or_toplevel_const(&exception)
        && const_name(&exception).as_deref() == Some("StandardError")
    {
        Some(exception)
    } else {
        None
    }
}

/// Enforces that `rescue StandardError` is written either explicitly or
/// implicitly, per `EnforcedStyle`.
#[derive(Debug, Clone)]
pub struct RescueStandardError {
    style: Style,
}

impl Rule for RescueStandardError {
    const META: RuleMeta = RuleMeta {
        name: "Style/RescueStandardError",
        department: Department::Style,
        summary: "Avoid rescuing without specifying an error class.",
        explanation: "\
Checks for rescuing `StandardError`. There are two supported
styles `implicit` and `explicit`. This cop will not register an offense
if any error other than `StandardError` is specified.

```ruby
# EnforcedStyle: explicit (default)
# `explicit` will enforce using `rescue StandardError`
# instead of `rescue`.

# bad
begin
  foo
rescue
  bar
end

# good
begin
  foo
rescue StandardError
  bar
end

# EnforcedStyle: implicit
# `implicit` will enforce using `rescue` instead of
# `rescue StandardError`.

# bad
begin
  foo
rescue StandardError
  bar
end

# good
begin
  foo
rescue
  bar
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::RescueNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("explicit"),
            allowed: &["implicit", "explicit"],
            doc: "Whether to enforce `rescue` or `rescue StandardError` for the bare case.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "implicit" => Style::Implicit,
            _ => Style::Explicit,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let clause = node.as_rescue_node().expect("kind matched");
        match self.style {
            Style::Implicit => {
                if let Some(error) = standard_error_only(&clause) {
                    let keyword_span = clause.keyword_loc().span();
                    let span = Span::new(keyword_span.start, error.span().end);
                    let fix = Fix {
                        applicability: Applicability::Safe,
                        edits: vec![Edit::delete(Span::new(keyword_span.end, error.span().end))],
                    };
                    ctx.report_with_fix(&Self::META, span, MSG_IMPLICIT, fix);
                }
            }
            Style::Explicit => {
                if clause.exceptions().is_empty() {
                    let keyword_span = clause.keyword_loc().span();
                    let fix = Fix {
                        applicability: Applicability::Safe,
                        edits: vec![Edit::insert(keyword_span.end, b" StandardError".to_vec())],
                    };
                    ctx.report_with_fix(&Self::META, keyword_span, MSG_EXPLICIT, fix);
                }
            }
        }
    }
}
