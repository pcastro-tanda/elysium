//! `Style/ModuleMemberExistenceCheck`, ported from RuboCop's
//! `lib/rubocop/cop/style/module_member_existence_check.rb`.
//!
//! Upstream's `on_send` fires on the *inner* call (`RESTRICT_ON_SEND`'s
//! `instance_methods`/`class_variables`/etc.) and inspects `node.parent` for
//! the `include?`/`member?` wrapper. Without a parent-node accessor here,
//! the walk is flipped: `enter` fires on the *outer* `include?`/`member?`
//! call and inspects its `receiver` for the qualifying inner call instead --
//! equivalent since the pattern requires that exact direct-receiver
//! relationship either way.
//!
//! whitequark's `block_argument?`/`splat_argument?`/`first_argument&
//! .hash_type?` guards (`simple_method_argument?`) are re-derived from
//! Prism's shapes: a bare `&foo` argument lands in `CallNode::block` (a
//! `BlockArgumentNode`) rather than in `arguments`, a positional splat shows
//! up as `ArgumentsNode::is_contains_splat`, and `**foo` alone becomes a
//! sole `KeywordHashNode` argument (so the existing `hash_type?`-style check
//! already covers it without a separate kwsplat case).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `METHOD_REPLACEMENTS`.
fn replacement_method(name: &[u8]) -> Option<&'static str> {
    match name {
        b"class_variables" => Some("class_variable_defined?"),
        b"instance_methods" => Some("method_defined?"),
        b"private_instance_methods" => Some("private_method_defined?"),
        b"protected_instance_methods" => Some("protected_method_defined?"),
        b"public_instance_methods" => Some("public_method_defined?"),
        _ => None,
    }
}

/// `METHODS_WITHOUT_INHERIT_PARAM`.
fn without_inherit_param(name: &[u8]) -> bool {
    name == b"class_variables"
}

/// Checks for usage of `Module` methods returning arrays that can be
/// replaced with equivalent predicates.
#[derive(Debug, Clone)]
pub struct ModuleMemberExistenceCheck;

impl Rule for ModuleMemberExistenceCheck {
    const META: RuleMeta = RuleMeta {
        name: "Style/ModuleMemberExistenceCheck",
        department: Department::Style,
        summary: "Checks for usage of `Module` methods returning arrays that can be replaced with equivalent predicates.",
        explanation: "",
        enabled_by_default: false,
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
        let Some(outer) = node.as_call_node() else { return };
        let name = outer.name();
        if !matches!(name.as_slice(), b"include?" | b"member?") {
            return;
        }
        let Some(outer_args) = outer.arguments() else { return };
        if outer_args.arguments().len() != 1 {
            return;
        }
        let Some(receiver) = outer.receiver() else { return };
        let Some(inner) = receiver.as_call_node() else { return };
        let Some(replacement_name) = replacement_method(inner.name().as_slice()) else { return };

        let inner_args_count = inner.arguments().map_or(0, |a| a.arguments().len());
        if without_inherit_param(inner.name().as_slice()) {
            if inner_args_count != 0 {
                return;
            }
        } else if inner_args_count > 1 {
            return;
        }

        if !simple_method_argument(&inner) || !simple_method_argument(&outer) {
            return;
        }

        let Some(selector) = inner.message_loc() else { return };
        let offense_range = Span::new(selector.span().start, outer.as_node().span().end);

        let outer_first_arg = outer_args.arguments().first().expect("checked len == 1");
        let outer_arg_source = ctx.text(outer_first_arg.span());
        let outer_arg_source = String::from_utf8_lossy(outer_arg_source);

        let inner_first_arg = inner.arguments().and_then(|a| a.arguments().first());
        let replacement = match &inner_first_arg {
            _ if without_inherit_param(inner.name().as_slice()) => {
                format!("{replacement_name}({outer_arg_source})")
            }
            None => format!("{replacement_name}({outer_arg_source})"),
            Some(arg) if arg.as_true_node().is_some() => {
                format!("{replacement_name}({outer_arg_source})")
            }
            Some(arg) => {
                let inner_arg_source = String::from_utf8_lossy(ctx.text(arg.span()));
                format!("{replacement_name}({outer_arg_source}, {inner_arg_source})")
            }
        };

        let message = format!("Use `{replacement}` instead.");
        ctx.report_with_fix(
            &Self::META,
            offense_range,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(offense_range, replacement.into_bytes())],
            },
        );
    }
}

/// RuboCop's `simple_method_argument?`.
fn simple_method_argument(call: &CallNode<'_>) -> bool {
    if call.block().is_some_and(|b| b.as_block_argument_node().is_some()) {
        return false;
    }
    if let Some(args) = call.arguments() {
        if args.is_contains_splat() {
            return false;
        }
        if args.arguments().first().is_some_and(|a| is_hash_like(&a)) {
            return false;
        }
    }
    true
}

/// whitequark's `hash_type?`: either a braced hash literal or a braceless
/// keyword-hash.
fn is_hash_like(node: &Node<'_>) -> bool {
    node.as_hash_node().is_some() || node.as_keyword_hash_node().is_some()
}
