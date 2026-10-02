//! `Rails/PluckInWhere`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/pluck_in_where.rb` (with `in_where?` from
//! `lib/rubocop/cop/mixin/active_record_helper.rb`).
//!
//! `in_where?` looks at `node.each_ancestor(:call).first`. RuboCop-AST's
//! block node sits between a call and its block body, so the call owning a
//! block is not an ancestor of what the block holds; Prism nests the body in
//! the call. The rule therefore keeps its own stack of enclosing calls and
//! skips those whose block contains the node asked about.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_SELECT: &str = "Use `select` instead of `pluck` within `where` query method.";
const MSG_IDS: &str = "Use `select(:id)` instead of `ids` within `where` query method.";

/// What `in_where?` needs to know about one enclosing call.
#[derive(Debug, Clone, Copy)]
struct Enclosing {
    /// `WHERE_METHODS.include?(method_name)`.
    where_method: bool,
    /// `method?(:not)`.
    not_method: bool,
    /// The receiver's span.
    receiver: Option<Span>,
    /// `receiver.send_type? && WHERE_METHODS.include?(receiver.method_name)`.
    receiver_is_where_send: bool,
    /// The call's literal block, which holds nodes that are not its operands.
    block: Option<Span>,
}

/// Identifies places where `pluck` is used within `where` query methods and
/// can be replaced with `select`.
#[derive(Debug, Clone)]
pub struct PluckInWhere {
    aggressive: bool,
    stack: Vec<Enclosing>,
}

impl Rule for PluckInWhere {
    const META: RuleMeta = RuleMeta {
        name: "Rails/PluckInWhere",
        department: Department::Rails,
        summary: "Use `select` instead of `pluck` in `where` query methods.",
        explanation: "Identifies places where `pluck` is used in `where` query methods and \
                      suggests using `select` instead.\n\nSince `pluck` is an eager method and \
                      hits the database immediately, using `select` helps to avoid additional \
                      database queries.\n\nThis cop has two different enforcement modes. When \
                      the EnforcedStyle is conservative (the default) then only calls to \
                      `pluck` on a constant (i.e. a model class) in the `where` is used as \
                      offenses.\n\nWhen the EnforcedStyle is aggressive then all calls to \
                      `pluck` in the `where` are considered offenses.\n\nThis cop's \
                      autocorrection is unsafe because it may change the result when the \
                      receiver is not an Active Record relation.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("conservative"),
            allowed: &["conservative", "aggressive"],
            doc: "`conservative` only reports `pluck` called on a constant.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { aggressive: options.style("EnforcedStyle")? == "aggressive", stack: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.stack.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        self.check(&call, node.span(), ctx);

        let receiver = call.receiver();
        let name = call.name();
        self.stack.push(Enclosing {
            where_method: is_where_method(name.as_slice()),
            not_method: name.as_slice() == b"not",
            receiver: receiver.as_ref().map(ruby_ast::NodeExt::span),
            receiver_is_where_send: receiver
                .and_then(|r| r.as_call_node().map(|c| is_where_send(&c)))
                .unwrap_or(false),
            block: call.block().and_then(|b| b.as_block_node().map(|b| b.location().span())),
        });
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.as_call_node().is_some() {
            self.stack.pop();
        }
    }
}

fn is_where_method(name: &[u8]) -> bool {
    matches!(name, b"where" | b"rewhere")
}

/// `receiver.send_type? && WHERE_METHODS.include?(receiver.method_name)`.
fn is_where_send(call: &CallNode<'_>) -> bool {
    !call.is_safe_navigation() && is_where_method(call.name().as_slice())
}

impl PluckInWhere {
    fn check(&self, call: &CallNode<'_>, span: Span, ctx: &mut Context<'_>) {
        let name = call.name();
        let ids = match name.as_slice() {
            b"pluck" => false,
            b"ids" => true,
            _ => return,
        };
        if !self.in_where(span) {
            return;
        }
        if !self.aggressive && !root_receiver_is_const(call) {
            return;
        }
        let Some(selector) = call.message_loc().map(|loc| loc.span()) else { return };
        let (replacement, message) =
            if ids { ("select(:id)", MSG_IDS) } else { ("select", MSG_SELECT) };
        ctx.report_with_fix(
            &Self::META,
            selector,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(selector, replacement.as_bytes().to_vec())],
            },
        );
    }

    /// `in_where?`.
    fn in_where(&self, span: Span) -> bool {
        let Some(send_node) = self.stack.iter().rev().find(|enclosing| {
            !enclosing.block.is_some_and(|block| block.start <= span.start && span.end <= block.end)
        }) else {
            return false;
        };
        if send_node.where_method && send_node.receiver != Some(span) {
            return true;
        }
        send_node.not_method && send_node.receiver_is_where_send
    }
}

/// `root_receiver(node)&.const_type?`.
fn root_receiver_is_const(call: &CallNode<'_>) -> bool {
    let mut receiver = call.receiver();
    while let Some(node) = receiver {
        let Some(inner) = node.as_call_node() else {
            return node.as_constant_read_node().is_some()
                || node.as_constant_path_node().is_some();
        };
        // `Model.where { }`: RuboCop's receiver is the `block` node here.
        if inner.block().is_some_and(|b| b.as_block_node().is_some()) {
            return false;
        }
        receiver = inner.receiver();
    }
    false
}
