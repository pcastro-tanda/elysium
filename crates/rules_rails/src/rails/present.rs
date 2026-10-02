//! `Rails/Present`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/present.rb`.
//!
//! Upstream's `on_or` can never fire (`exists_and_not_empty?` only matches an
//! `and` node), so it is not reproduced. Node equality of the two captured
//! variables is approximated by source-text equality.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Enforces use of `present?`.
#[derive(Debug, Clone)]
pub struct Present {
    not_nil_and_not_empty: bool,
    not_blank: bool,
    unless_blank: bool,
    /// `config.cop_enabled?('Style/UnlessElse')`.
    unless_else: UnlessElse,
}

/// Whether `Style/UnlessElse` is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnlessElse {
    Enabled,
    Disabled,
}

/// A captured variable: `None` when the matched send has no receiver.
#[derive(Debug, Clone, Copy)]
struct Subject<'pr>(Option<Node<'pr>>);

impl Rule for Present {
    const META: RuleMeta = RuleMeta {
        name: "Rails/Present",
        department: Department::Rails,
        summary: "Enforces use of `present?`.",
        explanation: "Checks for code that can be written with simpler conditionals using \
                      `Object#present?` defined by Active Support.\n\nInteraction with \
                      `Style/UnlessElse`: the configuration of `NotBlank` will not produce an \
                      offense in the context of `unless else` if `Style/UnlessElse` is \
                      enabled. This is to prevent interference between the autocorrection of \
                      the two cops.\n\n```ruby\n# bad\n!foo.nil? && !foo.empty?\n\n# bad\nfoo != \
                      nil && !foo.empty?\n\n# bad\n!foo.blank?\n\n# bad\nnot foo.blank?\n\n# \
                      bad\nsomething unless foo.blank?\n\n# good\nfoo.present?\n\n# good\n\
                      something if foo.present?\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode, NodeKind::AndNode, NodeKind::UnlessNode],
        config: &[
            ConfigOption {
                name: "NotNilAndNotEmpty",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Convert usages of `!nil? && !empty?` to `present?`.",
            },
            ConfigOption {
                name: "NotBlank",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Convert usages of `!blank?` to `present?`.",
            },
            ConfigOption {
                name: "UnlessBlank",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Convert usages of `unless blank?` to `if present?`.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            not_nil_and_not_empty: options.bool("NotNilAndNotEmpty"),
            not_blank: options.bool("NotBlank"),
            unless_blank: options.bool("UnlessBlank"),
            unless_else: if matches!(
                options.peer("Style/UnlessElse", "Enabled"),
                Some(OptionValue::Bool(false))
            ) {
                UnlessElse::Disabled
            } else {
                UnlessElse::Enabled
            },
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => self.on_send(node, ctx),
            NodeKind::AndNode => self.on_and(node, ctx),
            NodeKind::UnlessNode => self.on_unless(node, ctx),
            _ => {}
        }
    }
}

impl Present {
    fn on_send(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.not_blank {
            return;
        }
        let Some(receiver) = not_blank(node) else { return };
        let message = format!(
            "Use `{}` instead of `{}`.",
            replacement(ctx, receiver),
            String::from_utf8_lossy(ctx.text(node.span()))
        );
        report(ctx, node.span(), message, node.span(), receiver);
    }

    fn on_and(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.not_nil_and_not_empty {
            return;
        }
        let Some(and) = node.as_and_node() else { return };
        let Some(var) = exists_and_not_empty(ctx, &and.left(), &and.right()) else { return };
        let message = format!(
            "Use `{}` instead of `{}`.",
            replacement(ctx, var),
            String::from_utf8_lossy(ctx.text(node.span()))
        );
        report(ctx, node.span(), message, node.span(), var);
    }

    fn on_unless(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.unless_blank {
            return;
        }
        let Some(unless) = node.as_unless_node() else { return };
        if unless.else_clause().is_some() && self.unless_else == UnlessElse::Enabled {
            return;
        }
        let predicate = unless.predicate();
        let Some(call) = predicate.as_call_node() else { return };
        // `(send $_ :blank?)`
        if call.name().as_slice() != b"blank?"
            || call.is_safe_navigation()
            || call.arguments().is_some()
            || call.block().is_some()
        {
            return;
        }
        let receiver = Subject(call.receiver());
        let keyword = unless.keyword_loc().span();
        let range = if unless.end_keyword_loc().is_none() {
            Span::new(keyword.start, node.span().end)
        } else {
            Span::new(node.span().start, predicate.span().end)
        };
        let message = format!(
            "Use `if {}` instead of `{}`.",
            replacement(ctx, receiver),
            String::from_utf8_lossy(ctx.text(range))
        );
        let replacement = replacement(ctx, receiver);
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::replace(keyword, b"if".to_vec()),
                    Edit::replace(predicate.span(), replacement.into_bytes()),
                ],
            },
        );
    }
}

fn report(ctx: &mut Context<'_>, span: Span, message: String, replace: Span, var: Subject<'_>) {
    let replacement = replacement(ctx, var);
    ctx.report_with_fix(
        &Present::META,
        span,
        message,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(replace, replacement.into_bytes())],
        },
    );
}

/// `(send (send $_ :blank?) :!)`; the outer `Option` is the match, the inner
/// the (possibly absent) receiver.
fn not_blank<'pr>(node: &Node<'pr>) -> Option<Subject<'pr>> {
    let call = node.as_call_node()?;
    let inner = bang_target(&call)?;
    let inner = inner.as_call_node()?;
    if !is_plain(&inner, b"blank?") {
        return None;
    }
    Some(Subject(inner.receiver()))
}

/// The receiver of a plain `!x` send.
fn bang_target<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    if !is_plain(call, b"!") {
        return None;
    }
    call.receiver()
}

/// A `send` of `name` with no arguments, no block and no safe navigation.
fn is_plain(call: &CallNode<'_>, name: &[u8]) -> bool {
    call.name().as_slice() == name
        && !call.is_safe_navigation()
        && call.arguments().is_none()
        && call.block().is_none()
}

/// `exists_and_not_empty?`, followed by the `var1 == var2` check.
fn exists_and_not_empty<'pr>(
    ctx: &Context<'_>,
    left: &Node<'pr>,
    right: &Node<'pr>,
) -> Option<Subject<'pr>> {
    // Right: `(send (send $_ :empty?) :!)`.
    let right_call = right.as_call_node()?;
    let empty = bang_target(&right_call)?;
    let empty = empty.as_call_node()?;
    if !is_plain(&empty, b"empty?") {
        return None;
    }
    let var2 = empty.receiver();
    let var1 = left_variable(left);
    let same = match (&var1.0, &var2) {
        (Some(a), Some(b)) => ctx.text(a.span()) == ctx.text(b.span()),
        (None, None) => true,
        _ => false,
    };
    same.then_some(var1)
}

/// The alternatives for the left side of the `and`, in upstream's order; the
/// last (`$_`) captures the whole node.
fn left_variable<'pr>(left: &Node<'pr>) -> Subject<'pr> {
    if let Some(call) = left.as_call_node() {
        // `(send (send $_ :nil?) :!)` and `(send (send $_ :!) :!)`
        if let Some(inner) = bang_target(&call).and_then(|inner| inner.as_call_node()) {
            if is_plain(&inner, b"nil?") || is_plain(&inner, b"!") {
                return Subject(inner.receiver());
            }
        }
        // `(send $_ :!= nil)`
        if call.name().as_slice() == b"!="
            && !call.is_safe_navigation()
            && call.block().is_none()
            && call.arguments().is_some_and(|args| {
                let args = args.arguments();
                args.len() == 1 && args.first().is_some_and(|a| a.as_nil_node().is_some())
            })
        {
            return Subject(call.receiver());
        }
    }
    Subject(Some(*left))
}

/// `replacement`: `"<source>.present?"`, or `present?` without a receiver.
fn replacement(ctx: &Context<'_>, subject: Subject<'_>) -> String {
    subject.0.map_or_else(
        || "present?".to_owned(),
        |node| format!("{}.present?", String::from_utf8_lossy(ctx.text(node.span()))),
    )
}
