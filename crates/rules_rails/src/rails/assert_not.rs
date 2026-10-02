//! `Rails/AssertNot`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/assert_not.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{Node, NodeKind};

const MSG: &str = "Prefer `assert_not` over `assert !`.";

/// Use `assert_not` instead of `assert !`.
#[derive(Debug, Clone)]
pub struct AssertNot;

impl Rule for AssertNot {
    const META: RuleMeta = RuleMeta {
        name: "Rails/AssertNot",
        department: Department::Rails,
        summary: "Use `assert_not` instead of `assert !`.",
        explanation: "Use `assert_not` instead of `assert !`.\n\n```ruby\n# bad\nassert !x\n\n\
                      # good\nassert_not x\n```",
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
        // `(send nil? :assert (send ... :!) ...)`.
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"assert"
            || call.receiver().is_some()
            || call.is_safe_navigation()
        {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let Some(first) = arguments.arguments().iter().next() else { return };
        let Some(negation) = first.as_call_node() else { return };
        if negation.name().as_slice() != b"!"
            || negation.receiver().is_none()
            || negation.arguments().is_some()
        {
            return;
        }

        let span = call_span_excluding_block(&call);
        match corrected_source(ctx.text(span)) {
            Some(corrected) => ctx.report_with_fix(
                &Self::META,
                span,
                MSG,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(span, corrected)],
                },
            ),
            None => ctx.report(&Self::META, span, MSG),
        }
    }
}

/// `source.gsub(/^assert(\(| ) *! */, 'assert_not\\1')`; `None` when the
/// pattern does not match (for example `assert not x`).
fn corrected_source(source: &[u8]) -> Option<Vec<u8>> {
    let rest = source.strip_prefix(b"assert")?;
    let (&separator, rest) = rest.split_first()?;
    if separator != b'(' && separator != b' ' {
        return None;
    }
    let rest = skip_spaces(rest);
    let rest = rest.strip_prefix(b"!")?;
    let rest = skip_spaces(rest);
    let mut corrected = b"assert_not".to_vec();
    corrected.push(separator);
    corrected.extend_from_slice(rest);
    Some(corrected)
}

fn skip_spaces(bytes: &[u8]) -> &[u8] {
    let skip = bytes.iter().take_while(|&&byte| byte == b' ').count();
    &bytes[skip..]
}
