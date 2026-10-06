//! `Rails/Blank`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/blank.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_NIL_OR_EMPTY: &str = "Use `{prefer}` instead of `{current}`.";
const MSG_UNLESS_PRESENT: &str = "Use `if {prefer}` instead of `{current}`.";

/// Whether `Style/UnlessElse` is enabled (`config.cop_enabled?`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnlessElse {
    Enabled,
    Disabled,
}

/// A captured receiver; `None` when absent (a bare `nil?`).
struct Capture<'pr>(Option<Node<'pr>>);

/// Enforces use of `blank?`.
#[derive(Debug, Clone)]
pub struct Blank {
    nil_or_empty: bool,
    not_present: bool,
    unless_present: bool,
    unless_else: UnlessElse,
    /// The sole statement of a `def blank?` body (`defining_blank?`).
    defining_body: Option<Span>,
}

impl Rule for Blank {
    const META: RuleMeta = RuleMeta {
        name: "Rails/Blank",
        department: Department::Rails,
        summary: "Enforces use of `blank?`.",
        explanation: "Checks for code that can be written with simpler conditionals using \
                      `Object#blank?` defined by Active Support.\n\nInteraction with \
                      `Style/UnlessElse`: the configuration of `NotPresent` will not produce an \
                      offense in the context of `unless else` if `Style/UnlessElse` is enabled. \
                      This is to prevent interference between the autocorrection of the two \
                      cops.\n\nThis cop is unsafe autocorrection, because `' '.empty?` returns \
                      false, but `' '.blank?` returns true. Therefore, autocorrection is not \
                      compatible if the receiver is a non-empty blank string, tab, or newline \
                      meta characters.\n\n```ruby\n# bad\nfoo.nil? || foo.empty?\nfoo == nil || \
                      foo.empty?\n!foo.present?\nsomething unless foo.present?\n\n# good\n\
                      foo.blank?\nsomething if foo.blank?\n\ndef blank?\n  !present?\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::OrNode, NodeKind::UnlessNode, NodeKind::DefNode],
        config: &[
            ConfigOption {
                name: "NilOrEmpty",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Convert usages of `nil? || empty?` to `blank?`.",
            },
            ConfigOption {
                name: "NotPresent",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Convert usages of `!present?` to `blank?`.",
            },
            ConfigOption {
                name: "UnlessPresent",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Convert usages of `unless present?` to `if blank?`.",
            },
        ],
        blind_spots: "Receiver equality in `nil? || empty?` is compared by source text.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            nil_or_empty: options.bool("NilOrEmpty"),
            not_present: options.bool("NotPresent"),
            unless_present: options.bool("UnlessPresent"),
            unless_else: if matches!(
                options.peer("Style/UnlessElse", "Enabled"),
                Some(OptionValue::Bool(false))
            ) {
                UnlessElse::Disabled
            } else {
                UnlessElse::Enabled
            },
            defining_body: None,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::DefNode => self.note_def(node),
            NodeKind::CallNode => self.on_send(node, ctx),
            NodeKind::OrNode => self.on_or(node, ctx),
            NodeKind::UnlessNode => self.on_unless(node, ctx),
            _ => {}
        }
    }
}

impl Blank {
    /// `(def :blank? (args) ...)` whose body is one statement: that statement
    /// is a child of the `def` itself in whitequark.
    fn note_def(&mut self, node: &Node<'_>) {
        let Some(def) = node.as_def_node() else { return };
        if def.name().as_slice() != b"blank?" || def.receiver().is_some() {
            return;
        }
        if def.parameters().is_some() {
            return;
        }
        let Some(body) = def.body() else { return };
        let Some(statements) = body.as_statements_node() else { return };
        let mut iter = statements.body().iter();
        if let (Some(only), None) = (iter.next(), iter.next()) {
            self.defining_body = Some(only.span());
        }
    }

    fn on_send(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.not_present {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        // `(send (send $_ :present?) :!)`
        if call.name().as_slice() != b"!" || !is_plain_send(&call, false) {
            return;
        }
        let Some(inner_node) = call.receiver() else { return };
        let Some(inner) = inner_node.as_call_node() else { return };
        if inner.name().as_slice() != b"present?" || !is_plain_send(&inner, true) {
            return;
        }
        if self.defining_body == Some(node.span()) {
            return;
        }
        let receiver = inner.receiver();
        let prefer = replacement(ctx, receiver.as_ref());
        let current = String::from_utf8_lossy(ctx.text(node.span())).into_owned();
        let message = MSG_NIL_OR_EMPTY.replace("{prefer}", &prefer).replace("{current}", &current);
        let span = node.span();
        report(ctx, span, message, vec![Edit::replace(span, prefer.into_bytes())]);
    }

    fn on_or(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.nil_or_empty {
            return;
        }
        let Some(or) = node.as_or_node() else { return };
        let Some(var1) = left_variable(&or.left()) else { return };
        let Some(var2) = right_variable(&or.right()) else { return };
        let same = match (&var1.0, &var2.0) {
            (Some(a), Some(b)) => ctx.text(a.span()) == ctx.text(b.span()),
            (None, None) => true,
            _ => false,
        };
        if !same {
            return;
        }
        let prefer = replacement(ctx, var1.0.as_ref());
        let current = String::from_utf8_lossy(ctx.text(node.span())).into_owned();
        let message = MSG_NIL_OR_EMPTY.replace("{prefer}", &prefer).replace("{current}", &current);
        let span = node.span();
        report(ctx, span, message, vec![Edit::replace(span, prefer.into_bytes())]);
    }

    fn on_unless(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.unless_present {
            return;
        }
        let Some(unless) = node.as_unless_node() else { return };
        if unless.else_clause().is_some() && self.unless_else == UnlessElse::Enabled {
            return;
        }
        // `(:if $(send $_ :present?) ...)`
        let predicate = unless.predicate();
        let Some(method_call) = predicate.as_call_node() else { return };
        if method_call.name().as_slice() != b"present?" || !is_plain_send(&method_call, true) {
            return;
        }
        let receiver = method_call.receiver();
        let keyword = unless.keyword_loc().span();
        let range = Span::new(keyword.start, predicate.span().end);
        let prefer = replacement(ctx, receiver.as_ref());
        let current = String::from_utf8_lossy(ctx.text(range)).into_owned();
        let message =
            MSG_UNLESS_PRESENT.replace("{prefer}", &prefer).replace("{current}", &current);
        report(
            ctx,
            range,
            message,
            vec![
                Edit::replace(keyword, b"if".to_vec()),
                Edit::replace(predicate.span(), prefer.into_bytes()),
            ],
        );
    }
}

fn report(ctx: &mut Context<'_>, span: Span, message: String, edits: Vec<Edit>) {
    ctx.report_with_fix(
        &Blank::META,
        span,
        message,
        Fix { applicability: Applicability::Unsafe, edits },
    );
}

/// A whitequark `send` with no arguments and no attached block; `receiver_ok`
/// says whether a missing receiver is acceptable (`$_` matches `nil`).
fn is_plain_send(call: &CallNode<'_>, receiver_ok: bool) -> bool {
    !call.is_safe_navigation()
        && call.arguments().is_none()
        && call.block().is_none()
        && (receiver_ok || call.receiver().is_some())
}

/// `replacement`: `"#{node.source}.blank?"`, or `blank?` without a receiver.
fn replacement(ctx: &Context<'_>, node: Option<&Node<'_>>) -> String {
    node.map_or_else(
        || "blank?".to_string(),
        |node| format!("{}.blank?", String::from_utf8_lossy(ctx.text(node.span()))),
    )
}

/// Left alternatives of `nil_or_empty?`; `None` is no match.
fn left_variable<'pr>(node: &Node<'pr>) -> Option<Capture<'pr>> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() || call.block().is_some() {
        return None;
    }
    let arguments: Vec<Node<'pr>> =
        call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
    match (call.name().as_slice(), arguments.as_slice()) {
        // (send $_ :!) -- the receiver must exist for `!` to be a call.
        (b"!" | b"nil?", []) => Some(Capture(call.receiver())),
        // (send $_ :== nil)
        (b"==", [argument]) if argument.as_nil_node().is_some() => Some(Capture(call.receiver())),
        // (send nil :== $_)
        (b"==", [argument]) if call.receiver().is_some_and(|r| r.as_nil_node().is_some()) => {
            Some(Capture(Some(*argument)))
        }
        _ => None,
    }
}

/// Right alternatives of `nil_or_empty?`.
fn right_variable<'pr>(node: &Node<'pr>) -> Option<Capture<'pr>> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() || call.block().is_some() || call.arguments().is_some() {
        return None;
    }
    match call.name().as_slice() {
        // (send $_ :empty?)
        b"empty?" => Some(Capture(call.receiver())),
        // (send (send (send $_ :empty?) :!) :!)
        b"!" => {
            let middle_node = call.receiver()?;
            let middle = middle_node.as_call_node()?;
            if middle.name().as_slice() != b"!" || !is_plain_send(&middle, false) {
                return None;
            }
            let inner_node = middle.receiver()?;
            let inner = inner_node.as_call_node()?;
            if inner.name().as_slice() != b"empty?" || !is_plain_send(&inner, true) {
                return None;
            }
            Some(Capture(inner.receiver()))
        }
        _ => None,
    }
}
