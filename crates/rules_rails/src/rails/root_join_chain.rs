//! `Rails/RootJoinChain`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/root_join_chain.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, const_name, is_bare_or_toplevel_const};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use a single `#join` instead of chaining on `Rails.root` or
/// `Rails.public_path`.
#[derive(Debug, Clone)]
pub struct RootJoinChain {
    /// Spans of the receivers and arguments of every `join` call entered so
    /// far: the nodes whose parent is a `join` send.
    join_children: Vec<Span>,
}

impl Rule for RootJoinChain {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RootJoinChain",
        department: Department::Rails,
        summary: "Use a single `#join` instead of chaining on `Rails.root` or `Rails.public_path`.",
        explanation: "Use a single `#join` instead of chaining on `Rails.root` or \
                      `Rails.public_path`.\n\n```ruby\n# bad\nRails.root.join('db').join('schema.rb')\n\
                      Rails.public_path.join('path').join(to).join('file.pdf')\n\n# good\n\
                      Rails.root.join('db', 'schema.rb')\nRails.public_path.join('path', to, \
                      'file.pdf')\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { join_children: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if !is_join(&call) {
            return;
        }
        // Whether this `join` is itself a receiver/argument of a `join`.
        let span = node.span();
        let has_join_parent = self.join_children.contains(&span);
        // Record this call's children for the nodes entered after it.
        if let Some(receiver) = call.receiver() {
            self.remember(&receiver);
        }
        if let Some(arguments) = call.arguments() {
            for argument in &arguments.arguments() {
                self.remember(&argument);
            }
        }
        if has_join_parent {
            return;
        }
        // Is there only one join?
        if call.receiver().is_some_and(|receiver| rails_root(&receiver).is_some()) {
            return;
        }

        let mut collected = argument_spans(&call);
        let Some(mut current) = call.receiver() else { return };
        while let Some(inner) = current.as_call_node() {
            // An inner call with a literal block is a `block`, not a `send`.
            if !is_join(&inner) || inner.block().is_some_and(|b| b.as_block_node().is_some()) {
                break;
            }
            let mut spans = argument_spans(&inner);
            spans.append(&mut collected);
            collected = spans;
            let Some(receiver) = inner.receiver() else { return };
            current = receiver;
        }
        let rails = current;
        let Some(root_selector_end) = rails_root(&rails) else { return };
        let root_source = String::from_utf8_lossy(ctx.text(rails.span())).into_owned();
        let args = collected
            .iter()
            .map(|span| String::from_utf8_lossy(ctx.text(*span)).into_owned())
            .collect::<Vec<_>>()
            .join(", ");
        let call_span = call_span_excluding_block(&call);
        let range = Span::new(root_selector_end, call_span.end);
        ctx.report_with_fix(
            &Self::META,
            call_span,
            format!("Use `{root_source}.join(...)` instead of chaining `#join` calls."),
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, format!(".join({args})").into_bytes())],
            },
        );
    }
}

impl RootJoinChain {
    fn remember(&mut self, child: &Node<'_>) {
        // A child that is a call with a literal block is a `block` node in
        // whitequark, so its `send` has no `join` parent.
        if child
            .as_call_node()
            .is_some_and(|c| c.block().is_some_and(|b| b.as_block_node().is_some()))
        {
            return;
        }
        self.join_children.push(child.span());
    }
}

/// The `$...` of `(send _ :join $...)`: the arguments, a `&block` argument
/// included.
fn argument_spans(call: &ruby_ast::node::CallNode<'_>) -> Vec<Span> {
    let mut spans = Vec::new();
    if let Some(arguments) = call.arguments() {
        spans.extend(arguments.arguments().iter().map(|a| a.span()));
    }
    if let Some(block) = call.block() {
        if block.as_block_argument_node().is_some() {
            spans.push(block.span());
        }
    }
    spans
}

/// `(send _ :join ...)`.
fn is_join(call: &ruby_ast::node::CallNode<'_>) -> bool {
    call.name().as_slice() == b"join" && !call.is_safe_navigation()
}

/// `(send (const {nil? cbase} :Rails) {:root :public_path})`: the end of the
/// selector when `node` matches.
fn rails_root(node: &Node<'_>) -> Option<u32> {
    let call = node.as_call_node()?;
    if !matches!(call.name().as_slice(), b"root" | b"public_path")
        || call.is_safe_navigation()
        || call.arguments().is_some()
        || call.block().is_some()
    {
        return None;
    }
    let receiver = call.receiver()?;
    if !is_bare_or_toplevel_const(&receiver) || const_name(&receiver).as_deref() != Some("Rails") {
        return None;
    }
    Some(call.message_loc()?.span().end)
}
