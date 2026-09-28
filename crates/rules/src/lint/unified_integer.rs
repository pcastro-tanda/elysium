//! `Lint/UnifiedInteger`, ported from RuboCop's
//! `lib/rubocop/cop/lint/unified_integer.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use Integer instead of Fixnum or Bignum.
#[derive(Debug, Clone)]
pub struct UnifiedInteger {
    target_ruby_version: f32,
}

impl Rule for UnifiedInteger {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UnifiedInteger",
        department: Department::Lint,
        summary: "Use Integer instead of Fixnum or Bignum.",
        explanation: "Checks for using Fixnum or Bignum constant.\n\n\
            # bad\n\
            1.is_a?(Fixnum)\n\
            1.is_a?(Bignum)\n\n\
            # good\n\
            1.is_a?(Integer)\n",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ConstantReadNode, NodeKind::ConstantPathNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some((klass, name_span)) = fixnum_or_bignum_const(node) else { return };
        let msg = format!("Use `Integer` instead of `{klass}`.");

        if self.target_ruby_version <= 2.3 {
            ctx.report(&Self::META, node.span(), msg);
            return;
        }

        ctx.report_with_fix(
            &Self::META,
            node.span(),
            msg,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(name_span, b"Integer".to_vec())],
            },
        );
    }
}

/// RuboCop's `fixnum_or_bignum_const` node matcher: `(:const {nil? (:cbase)}
/// ${:Fixnum :Bignum})`. Returns the constant name and the span of just the
/// name part (excluding any leading `::`), matching `node.loc.name`.
fn fixnum_or_bignum_const(node: &Node<'_>) -> Option<(&'static str, Span)> {
    match node.kind() {
        NodeKind::ConstantReadNode => {
            let c = node.as_constant_read_node()?;
            let klass = match c.name().as_slice() {
                b"Fixnum" => "Fixnum",
                b"Bignum" => "Bignum",
                _ => return None,
            };
            Some((klass, node.span()))
        }
        NodeKind::ConstantPathNode => {
            let path = node.as_constant_path_node()?;
            if path.parent().is_some() {
                return None;
            }
            let name = path.name()?;
            let klass = match name.as_slice() {
                b"Fixnum" => "Fixnum",
                b"Bignum" => "Bignum",
                _ => return None,
            };
            Some((klass, path.name_loc().span()))
        }
        _ => None,
    }
}
