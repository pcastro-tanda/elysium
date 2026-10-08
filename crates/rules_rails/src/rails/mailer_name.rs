//! `Rails/MailerName`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/mailer_name.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Mailer should end with `Mailer` suffix.";

/// Enforces that mailer names end with `Mailer` suffix.
#[derive(Debug, Clone)]
pub struct MailerName {
    /// Name span and name of each enclosing constant assignment
    /// (`each_ancestor(:casgn).first` is the innermost).
    casgn_stack: Vec<(Span, Vec<u8>)>,
}

impl Rule for MailerName {
    const META: RuleMeta = RuleMeta {
        name: "Rails/MailerName",
        department: Department::Rails,
        summary: "Mailer should end with `Mailer` suffix.",
        explanation: "Enforces that mailer names end with `Mailer` suffix.\n\nWithout the \
                      `Mailer` suffix it isn't immediately apparent what's a mailer and which \
                      views are related to the mailer.\n\nThe autocorrection is unsafe because \
                      renaming a constant is always an unsafe operation.\n\n```ruby\n# bad\n\
                      class User < ActionMailer::Base\nend\n\nclass User < ApplicationMailer\n\
                      end\n\n# good\nclass UserMailer < ActionMailer::Base\nend\n\nclass \
                      UserMailer < ApplicationMailer\nend\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::ClassNode,
            NodeKind::CallNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantPathWriteNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { casgn_stack: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::ConstantWriteNode => {
                if let Some(write) = node.as_constant_write_node() {
                    self.casgn_stack
                        .push((write.name_loc().span(), write.name().as_slice().to_vec()));
                }
            }
            NodeKind::ConstantPathWriteNode => {
                if let Some(write) = node.as_constant_path_write_node() {
                    let target = write.target();
                    let name = target.name().map(|n| n.as_slice().to_vec()).unwrap_or_default();
                    self.casgn_stack.push((target.name_loc().span(), name));
                }
            }
            NodeKind::ClassNode => on_class(node, ctx),
            NodeKind::CallNode => self.on_send(node, ctx),
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if matches!(node.kind(), NodeKind::ConstantWriteNode | NodeKind::ConstantPathWriteNode) {
            self.casgn_stack.pop();
        }
    }
}

impl MailerName {
    fn on_send(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"new" || call.is_safe_navigation() {
            return;
        }
        if call.block().is_some_and(|b| b.as_block_argument_node().is_some()) {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if !is_bare_or_toplevel_const(&receiver)
            || const_name(&receiver).as_deref() != Some("Class")
        {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let mut arguments = arguments.arguments().iter();
        let (Some(argument), None) = (arguments.next(), arguments.next()) else { return };
        if !is_mailer_base_class(&argument) {
            return;
        }
        let Some((span, name)) = self.casgn_stack.last() else { return };
        if name.ends_with(b"Mailer") {
            return;
        }
        let mut replacement = name.clone();
        replacement.extend_from_slice(b"Mailer");
        report(ctx, *span, replacement);
    }
}

fn on_class(node: &Node<'_>, ctx: &mut Context<'_>) {
    let Some(class) = node.as_class_node() else { return };
    let name = class.name();
    if name.as_slice().ends_with(b"Mailer") {
        return;
    }
    let Some(superclass) = class.superclass() else { return };
    if !is_mailer_base_class(&superclass) {
        return;
    }
    let path = class.constant_path();
    // `(const _ name)`: a plain or qualified constant.
    if !matches!(path.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode) {
        return;
    }
    let mut replacement = name.as_slice().to_vec();
    replacement.extend_from_slice(b"Mailer");
    report(ctx, path.span(), replacement);
}

/// `mailer_base_class?`: `ActionMailer::Base` or `ApplicationMailer`, each
/// optionally rooted with `::`.
fn is_mailer_base_class(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::ConstantReadNode => const_name(node).as_deref() == Some("ApplicationMailer"),
        NodeKind::ConstantPathNode => {
            let Some(path) = node.as_constant_path_node() else { return false };
            match path.parent() {
                None => const_name(node).as_deref() == Some("ApplicationMailer"),
                Some(parent) => {
                    const_name(node).as_deref() == Some("ActionMailer::Base")
                        && is_bare_or_toplevel_const(&parent)
                }
            }
        }
        _ => false,
    }
}

fn report(ctx: &mut Context<'_>, span: Span, replacement: Vec<u8>) {
    ctx.report_with_fix(
        &MailerName::META,
        span,
        MSG,
        Fix { applicability: Applicability::Unsafe, edits: vec![Edit::replace(span, replacement)] },
    );
}
