//! `Style/RedundantInterpolationUnfreeze`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_interpolation_unfreeze.rb` plus the
//! `uninterpolated_string?`/`uninterpolated_heredoc?` helpers it borrows
//! from the `FrozenStringLiteral` mixin.
//!
//! RuboCop dispatches on `on_dstr` and inspects `node.parent` for the
//! `(send dstr_type? {:+@ :dup})` / `(send (const nil? :String) :new
//! dstr_type?)` shapes. This port dispatches on the containing
//! [`NodeKind::CallNode`] directly and inspects its receiver/argument for a
//! [`NodeKind::InterpolatedStringNode`] instead, which is equivalent and
//! avoids reaching through a parent pointer.
//!
//! `uninterpolated_string?`'s `each_descendant(:begin, :ivar, :cvar, :gvar)`
//! walks the whole dstr subtree for any actual interpolation; Prism's
//! equivalents are [`NodeKind::EmbeddedStatementsNode`] (`#{...}`) and
//! [`NodeKind::EmbeddedVariableNode`] (`#@ivar`/`#@@cvar`/`#$gvar`). Since
//! adjacent string-literal concatenation (`'foo' 'bar'`) also merges into an
//! outer `InterpolatedStringNode` in Prism (see
//! `style/implicit_string_concatenation.rs`), [`has_interpolation`] recurses
//! into nested `InterpolatedStringNode` parts the same way
//! `each_descendant` would. `uninterpolated_heredoc?`'s separate `dstr_type?
//! && heredoc? && children.all?(&:str_type?)` check is already subsumed by
//! this same "no actual interpolation" test, so it is not ported
//! separately.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str = "Don't unfreeze interpolated strings as they are already unfrozen.";

/// RuboCop's `uninterpolated_string?`/`uninterpolated_heredoc?`, negated:
/// does this interpolated string actually contain an embedded expression or
/// variable anywhere in its (possibly concatenation-nested) parts?
fn has_interpolation(node: &Node<'_>) -> bool {
    let Some(dstr) = node.as_interpolated_string_node() else { return false };
    dstr.parts().iter().any(|part| match part.kind() {
        NodeKind::EmbeddedStatementsNode | NodeKind::EmbeddedVariableNode => true,
        NodeKind::InterpolatedStringNode => has_interpolation(&part),
        _ => false,
    })
}

/// Checks for redundant unfreezing of interpolated strings.
///
/// Before Ruby 3.0, interpolated strings followed the frozen string literal
/// magic comment which sometimes made it necessary to explicitly unfreeze
/// them. Ruby 3.0 changed interpolated strings to always be unfrozen which
/// makes unfreezing them redundant.
///
/// # Examples
///
/// ```ruby
/// # bad
/// +"#{foo} bar"
///
/// # bad
/// "#{foo} bar".dup
///
/// # bad
/// String.new("#{foo} bar")
///
/// # good
/// "#{foo} bar"
/// ```
#[derive(Debug, Clone)]
pub struct RedundantInterpolationUnfreeze {
    minimum_target_ruby_version: f32,
}

impl Rule for RedundantInterpolationUnfreeze {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantInterpolationUnfreeze",
        department: Department::Style,
        summary: "Checks for redundant unfreezing of interpolated strings.",
        explanation: "\
Before Ruby 3.0, interpolated strings followed the frozen string literal \
magic comment which sometimes made it necessary to explicitly unfreeze them. \
Ruby 3.0 changed interpolated strings to always be unfrozen which makes \
unfreezing them redundant.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { minimum_target_ruby_version: options.target_ruby_version() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.minimum_target_ruby_version < 3.0 {
            return;
        }
        let call = node.as_call_node().expect("kind matched");
        let name = call.name();
        let name = name.as_slice();

        let offense_span = if name == b"new" {
            // `(send (const nil? :String) :new dstr_type?)`
            let Some(receiver) = call.receiver() else { return };
            if receiver.kind() != NodeKind::ConstantReadNode
                || ext::const_name(&receiver).as_deref() != Some("String")
            {
                return;
            }
            let Some(arguments) = call.arguments() else { return };
            let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
            let [arg] = args.as_slice() else { return };
            if !has_interpolation(arg) {
                return;
            }
            let Some(message_loc) = call.message_loc() else { return };
            ruby_source::Span::new(node.location().span().start, message_loc.span().end)
        } else if name == b"+@" || name == b"dup" {
            // `(send dstr_type? {:+@ :dup})`
            let Some(receiver) = call.receiver() else { return };
            if receiver.kind() != NodeKind::InterpolatedStringNode || !has_interpolation(&receiver)
            {
                return;
            }
            if call.arguments().is_some() {
                return;
            }
            let Some(message_loc) = call.message_loc() else { return };
            message_loc.span()
        } else {
            return;
        };

        // RuboCop's `corrector.replace(node, node.source)`: the receiver
        // keeps its own source, only the `+`/`.dup`/`String.new(...)`
        // wrapper is stripped away.
        let replacement_source = if name == b"new" {
            let arguments = call.arguments().expect("checked above");
            let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
            ctx.text(args[0].location().span()).to_vec()
        } else {
            let receiver = call.receiver().expect("checked above");
            ctx.text(receiver.location().span()).to_vec()
        };
        let fix = linter::Fix {
            applicability: linter::Applicability::Safe,
            edits: vec![linter::Edit::replace(node.location().span(), replacement_source)],
        };
        ctx.report_with_fix(&Self::META, offense_span, MSG, fix);
    }
}
