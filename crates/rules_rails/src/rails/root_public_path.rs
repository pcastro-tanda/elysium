//! `Rails/RootPublicPath`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/root_public_path.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, const_name, is_bare_or_toplevel_const};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Use `Rails.public_path`.";

/// Favor `Rails.public_path` over `Rails.root` with `'public'`.
#[derive(Debug, Clone)]
pub struct RootPublicPath;

impl Rule for RootPublicPath {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RootPublicPath",
        department: Department::Rails,
        summary: "Favor `Rails.public_path` over `Rails.root` with `'public'`.",
        explanation: "Favor `Rails.public_path` over `Rails.root` with `'public'`.\n\n```ruby\n# \
                      bad\nRails.root.join('public')\nRails.root.join('public/file.pdf')\n\
                      Rails.root.join('public', 'file.pdf')\n\n# good\nRails.public_path\n\
                      Rails.public_path.join('file.pdf')\nRails.public_path.join('file.pdf')\n```",
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
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"join" || call.is_safe_navigation() {
            return;
        }
        let Some(root) = call.receiver().and_then(|receiver| receiver.as_call_node()) else {
            return;
        };
        if root.name().as_slice() != b"root"
            || root.is_safe_navigation()
            || root.arguments().is_some()
        {
            return;
        }
        let Some(rails) = root.receiver() else { return };
        if !is_bare_or_toplevel_const(&rails) || const_name(&rails).as_deref() != Some("Rails") {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let mut arguments = arguments.arguments().iter();
        let Some(first) = arguments.next() else { return };
        let Some(string) = first.as_string_node() else { return };
        let Some(rest) = public_path_rest(string.unescaped()) else { return };

        let mut args: Vec<&[u8]> = Vec::new();
        let rest_arg = (!rest.is_empty()).then(|| {
            let mut quoted = Vec::with_capacity(rest.len() + 2);
            quoted.push(b'\'');
            quoted.extend_from_slice(rest);
            quoted.push(b'\'');
            quoted
        });
        let others: Vec<Node<'_>> = arguments.collect();
        let block_arg = call.block().filter(|block| block.as_block_argument_node().is_some());

        let mut replacement = ctx.text(rails.span()).to_vec();
        replacement.extend_from_slice(b".public_path");
        let mut joined: Vec<u8> = Vec::new();
        if let Some(quoted) = &rest_arg {
            joined.extend_from_slice(quoted);
        }
        args.extend(others.iter().map(|arg| ctx.text(arg.span())));
        args.extend(block_arg.iter().map(|block| ctx.text(block.span())));
        for arg in args {
            if !joined.is_empty() {
                joined.extend_from_slice(b", ");
            }
            joined.extend_from_slice(arg);
        }
        if !joined.is_empty() {
            replacement.extend_from_slice(b".join(");
            replacement.extend_from_slice(&joined);
            replacement.push(b')');
        }

        let span = call_span_excluding_block(&call);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement)],
            },
        );
    }
}

/// `PATTERN = %r{\Apublic(/|\z)}`: the string with that prefix removed, or
/// `None` when it does not match.
fn public_path_rest(string: &[u8]) -> Option<&[u8]> {
    let rest = string.strip_prefix(b"public")?;
    match rest.first() {
        None => Some(rest),
        Some(b'/') => Some(&rest[1..]),
        Some(_) => None,
    }
}
