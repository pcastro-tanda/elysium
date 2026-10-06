//! `Rails/EnvLocal`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/env_local.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `Rails.env.local?` instead.";
const MSG_NEGATED: &str = "Use `!Rails.env.local?` instead.";

/// `minimum_target_rails_version 7.1`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 7.1;

/// Which of `LOCAL_ENVIRONMENTS` a call named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Environment {
    Development,
    Test,
}

/// A matched `Rails.env.development?`/`test?` call, or the `!` wrapping one.
#[derive(Debug, Clone, Copy)]
struct Match {
    span: Span,
    environment: Environment,
}

/// Checks for usage of `Rails.env.development? || Rails.env.test?` which can
/// be replaced by `Rails.env.local?`.
#[derive(Debug, Clone, Copy)]
pub struct EnvLocal {
    supported: bool,
}

impl Rule for EnvLocal {
    const META: RuleMeta = RuleMeta {
        name: "Rails/EnvLocal",
        department: Department::Rails,
        summary: "Use `Rails.env.local?` instead of `Rails.env.development? || Rails.env.test?`.",
        explanation: "Checks for usage of `Rails.env.development? || Rails.env.test?` which can \
                      be replaced by `Rails.env.local?`, introduced in Rails 7.1.\n\n```ruby\n\
                      # bad\nRails.env.development? || Rails.env.test?\n\n# good\n\
                      Rails.env.local?\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::OrNode, NodeKind::AndNode],
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
        if let Some(or) = node.as_or_node() {
            check(&or.left(), &or.right(), false, ctx);
        } else if let Some(and) = node.as_and_node() {
            check(&and.left(), &and.right(), true, ctx);
        }
    }
}

/// `on_or` (`negated == false`) / `on_and`.
fn check(lhs: &Node<'_>, rhs: &Node<'_>, negated: bool, ctx: &mut Context<'_>) {
    let matcher = if negated { not_rails_env_local } else { rails_env_local };
    let Some(last) = matcher(rhs) else { return };

    let first = if let Some(first) = matcher(lhs) {
        first
    } else {
        // `lhs.or_type?` / `lhs.operator_keyword?` and `lhs.rhs`.
        let inner = if negated {
            lhs.as_or_node().map(|n| n.right()).or_else(|| lhs.as_and_node().map(|n| n.right()))
        } else {
            lhs.as_or_node().map(|n| n.right())
        };
        match inner.as_ref().and_then(matcher) {
            Some(first) => first,
            None => return,
        }
    };

    // `environments(nodes).to_set == LOCAL_ENVIRONMENTS`.
    if first.environment == last.environment {
        return;
    }
    let range = Span::new(first.span.start, last.span.end);
    let (message, replacement) =
        if negated { (MSG_NEGATED, "!Rails.env.local?") } else { (MSG, "Rails.env.local?") };
    ctx.report_with_fix(
        &EnvLocal::META,
        range,
        message,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(range, replacement.as_bytes().to_vec())],
        },
    );
}

/// `(send (send (const {cbase nil?} :Rails) :env) {development? test?})`.
fn rails_env_local(node: &Node<'_>) -> Option<Match> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() || call.arguments().is_some() {
        return None;
    }
    let environment = match call.name().as_slice() {
        b"development?" => Environment::Development,
        b"test?" => Environment::Test,
        _ => return None,
    };
    let env = call.receiver()?;
    let env = env.as_call_node()?;
    if env.is_safe_navigation() || env.arguments().is_some() || env.name().as_slice() != b"env" {
        return None;
    }
    let rails = env.receiver()?;
    if !is_bare_or_toplevel_const(&rails) || const_name(&rails).as_deref() != Some("Rails") {
        return None;
    }
    Some(Match { span: node.span(), environment })
}

/// `(send #rails_env_local? :!)`; the match spans the `!` call.
fn not_rails_env_local(node: &Node<'_>) -> Option<Match> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() || call.arguments().is_some() || call.name().as_slice() != b"!" {
        return None;
    }
    let inner = rails_env_local(&call.receiver()?)?;
    Some(Match { span: node.span(), environment: inner.environment })
}
