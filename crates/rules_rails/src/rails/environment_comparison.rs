//! `Rails/EnvironmentComparison`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/environment_comparison.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

use super::util;

const SYM_MSG: &str =
    "Do not compare `Rails.env` with a symbol, it will always evaluate to `false`.";

/// Checks that `Rails.env` is compared using `.production?`-like methods.
#[derive(Debug, Clone)]
pub struct EnvironmentComparison;

impl Rule for EnvironmentComparison {
    const META: RuleMeta = RuleMeta {
        name: "Rails/EnvironmentComparison",
        department: Department::Rails,
        summary: "Favor `Rails.env.production?` over `Rails.env == 'production'`.",
        explanation: "Checks that `Rails.env` is compared using `.production?`-like methods \
                      instead of equality against a string or symbol.\n\n```ruby\n# \
                      bad\nRails.env == 'production'\nRails.env.to_sym == :production\n\n# bad, \
                      always returns false\nRails.env == :test\n\n# good\nRails.env.production?\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        if !matches!(name.as_slice(), b"==" | b"!=") || call.is_safe_navigation() {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let args = util::parser_args(&call);
        let [argument] = args.as_slice() else { return };

        // Which side `Rails.env` is on, and the plain env node (`Rails.env`).
        let (other, env_side) = if is_rails_env(&receiver) || is_env_to_sym(&receiver) {
            (argument, &receiver)
        } else if is_rails_env(argument) || is_env_to_sym(argument) {
            (&receiver, argument)
        } else {
            return;
        };
        let to_sym = is_env_to_sym(env_side);
        let kind = other.kind();
        let literal_ok = if to_sym { kind == NodeKind::SymbolNode } else { is_str(other) };
        let sym_check = !to_sym && kind == NodeKind::SymbolNode;
        if !literal_ok && !sym_check {
            return;
        }
        let Some(replacement) = build_predicate(&call, ctx, other, env_side) else {
            return;
        };
        let source = String::from_utf8_lossy(ctx.text(node.span())).into_owned();
        let message = if literal_ok {
            format!("Favor `{replacement}` over `{source}`.")
        } else {
            SYM_MSG.to_string()
        };
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(node.span(), replacement.into_bytes())],
            },
        );
    }
}

/// `(send (const {nil? cbase} :Rails) :env)`.
fn is_rails_env(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.name().as_slice() != b"env"
        || call.is_safe_navigation()
        || !util::parser_args(&call).is_empty()
    {
        return false;
    }
    call.receiver().is_some_and(|r| {
        is_bare_or_toplevel_const(&r) && const_name(&r).as_deref() == Some("Rails")
    })
}

/// `(send (send (const {nil? cbase} :Rails) :env) :to_sym)`.
fn is_env_to_sym(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    call.name().as_slice() == b"to_sym"
        && !call.is_safe_navigation()
        && util::parser_args(&call).is_empty()
        && call.receiver().is_some_and(|r| is_rails_env(&r))
}

/// `str`: a plain, non-interpolated string literal.
fn is_str(node: &Node<'_>) -> bool {
    node.kind() == NodeKind::StringNode
}

/// `build_predicate_method`.
fn build_predicate(
    call: &CallNode<'_>,
    ctx: &Context<'_>,
    literal: &Node<'_>,
    env_side: &Node<'_>,
) -> Option<String> {
    let bang = if call.name().as_slice() == b"!=" { "!" } else { "" };
    let env = if is_env_to_sym(env_side) {
        env_side.as_call_node()?.receiver()?
    } else {
        env_side.as_call_node()?.as_node()
    };
    let value = match literal.kind() {
        NodeKind::StringNode => literal.as_string_node()?.unescaped().to_vec(),
        _ => literal.as_symbol_node()?.unescaped().to_vec(),
    };
    Some(format!(
        "{bang}{}.{}?",
        String::from_utf8_lossy(ctx.text(env.span())),
        String::from_utf8_lossy(&value)
    ))
}
