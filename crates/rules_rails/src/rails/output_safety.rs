//! `Rails/OutputSafety`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/output_safety.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str = "Tagging a string as html safe may be a security risk.";

/// The use of `html_safe` or `raw` may be a security risk.
#[derive(Debug, Clone)]
pub struct OutputSafety;

impl Rule for OutputSafety {
    const META: RuleMeta = RuleMeta {
        name: "Rails/OutputSafety",
        department: Department::Rails,
        summary: "The use of `html_safe` or `raw` may be a security risk.",
        explanation: "Checks for the use of output safety calls like `html_safe`, `raw`, and \
                      `safe_concat`. These methods do not escape content. They simply return a \
                      SafeBuffer containing the content as is. Instead, use `safe_join` to \
                      join content and escape it and concat to concatenate content and escape \
                      it, ensuring its safety.\n\n```ruby\n# bad\nuser_content = \
                      \"<b>hi</b>\"\nsafe = user_content.html_safe\nraw(user_content)\n\n# \
                      good\nsafe_join([\"<b>\".html_safe, user_content, \"</b>\".html_safe])\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
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
        let name = name.as_slice();
        if !matches!(name, b"html_safe" | b"raw" | b"safe_concat") {
            return;
        }
        let Some(selector) = call.message_loc() else { return };

        // A `&block` argument is an ordinary `send` argument in whitequark.
        let block_argument = call.block().filter(|block| block.as_block_argument_node().is_some());
        let argument_count = call.arguments().map_or(0, |args| args.arguments().iter().count())
            + usize::from(block_argument.is_some());

        let receiver = call.receiver();
        if receiver.as_ref().is_some_and(non_interpolated_string) {
            return;
        }

        // `def_node_search :i18n_method?`: the send itself and everything it
        // holds (receiver, arguments), but not a literal block attached to it.
        let mut found = false;
        let mut visit = |child: &Node<'_>| {
            if is_i18n_call(child) {
                found = true;
            }
        };
        if let Some(receiver) = &receiver {
            visit(receiver);
            each_descendant(receiver, &mut visit);
        }
        if let Some(arguments) = call.arguments() {
            for argument in &arguments.arguments() {
                visit(&argument);
                each_descendant(&argument, &mut visit);
            }
        }
        if let Some(block) = &block_argument {
            visit(block);
            each_descendant(block, &mut visit);
        }
        if found {
            return;
        }

        let offense = match name {
            b"html_safe" => receiver.is_some() && argument_count == 0,
            b"raw" => receiver.is_none() && argument_count == 1,
            _ => argument_count == 1,
        };
        if offense {
            ctx.report(&Self::META, selector.span(), MSG);
        }
    }
}

/// `receiver.str_type? || (receiver.dstr_type? && children.all?(&:str_type?))`.
fn non_interpolated_string(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::StringNode | NodeKind::SourceFileNode => true,
        NodeKind::InterpolatedStringNode => {
            node.as_interpolated_string_node().is_some_and(|string| {
                string.parts().iter().all(|part| part.as_string_node().is_some())
            })
        }
        _ => false,
    }
}

/// `(send {nil? (const {nil? cbase} :I18n)} {:t :translate :l :localize} ...)`.
fn is_i18n_call(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.is_safe_navigation()
        || !matches!(call.name().as_slice(), b"t" | b"translate" | b"l" | b"localize")
    {
        return false;
    }
    call.receiver().is_none_or(|receiver| {
        is_bare_or_toplevel_const(&receiver) && const_name(&receiver).as_deref() == Some("I18n")
    })
}
