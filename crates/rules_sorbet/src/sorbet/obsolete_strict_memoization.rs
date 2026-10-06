//! `Sorbet/ObsoleteStrictMemoization`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/obsolete_strict_memoization.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, GemVersion, OptionError,
    OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "This two-stage workaround for memoization in `#typed: strict` files is no longer necessary. See https://sorbet.org/docs/type-assertions#put-type-assertions-behind-memoization.";

/// Checks for the obsolete pattern for initializing instance variables that was required for older Sorbet versions in `#typed: strict` files.
#[derive(Debug, Clone)]
pub struct ObsoleteStrictMemoization {
    /// `relevant_file?`: the lockfile's `sorbet-static` is at least 0.5.10210.
    enabled: bool,
    /// `max_line_length`: `None` when `Layout/LineLength` is disabled.
    max_line_length: Option<usize>,
    /// Start offsets of `begin ... end` bodies (whitequark `kwbegin` holds
    /// its statements directly, so there is no `begin` node for them).
    kwbegin_bodies: Vec<u32>,
}

impl Rule for ObsoleteStrictMemoization {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ObsoleteStrictMemoization",
        department: Department::Sorbet,
        summary: "This cop checks for the obsolete pattern for initializing instance variables that was required for older Sorbet versions in `#typed: strict` files.",
        explanation: "Checks for the obsolete pattern for initializing instance variables that was required for older Sorbet\nversions in `#typed: strict` files.\n\nIt's no longer required, as of Sorbet 0.5.10210\nSee https://sorbet.org/docs/type-assertions#put-type-assertions-behind-memoization\n\n```ruby\n# bad\nsig { returns(Foo) }\ndef foo\n  @foo = T.let(@foo, T.nilable(Foo))\n  @foo ||= Foo.new\nend\n\n# bad\nsig { returns(Foo) }\ndef foo\n  # This would have been a mistake, causing the memoized value to be discarded and recomputed on every call.\n  @foo = T.let(nil, T.nilable(Foo))\n  @foo ||= Foo.new\nend\n\n# good\nsig { returns(Foo) }\ndef foo\n  @foo ||= T.let(Foo.new, T.nilable(Foo))\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StatementsNode, NodeKind::BeginNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let enabled = match (options.gem_version("sorbet-static"), GemVersion::parse("0.5.10210")) {
            (Some(locked), Some(minimum)) => minimum <= locked,
            _ => false,
        };
        let line_length_enabled =
            options.peer("Layout/LineLength", "Enabled").and_then(OptionValue::as_bool);
        let max_line_length = line_length_enabled.unwrap_or(true).then(|| {
            options
                .peer("Layout/LineLength", "Max")
                .and_then(OptionValue::as_int)
                .and_then(|max| usize::try_from(max).ok())
                .unwrap_or(120)
        });
        Ok(Self { enabled, max_line_length, kwbegin_bodies: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.enabled {
            return;
        }
        if let Some(begin) = node.as_begin_node() {
            if begin.begin_keyword_loc().is_some()
                && begin.rescue_clause().is_none()
                && begin.ensure_clause().is_none()
            {
                if let Some(statements) = begin.statements() {
                    self.kwbegin_bodies.push(statements.as_node().span().start);
                }
            }
            return;
        }
        let Some(statements) = node.as_statements_node() else { return };
        let stmts: Vec<Node<'_>> = statements.body().iter().collect();
        let [.., first, second] = stmts.as_slice() else { return };
        if self.kwbegin_bodies.contains(&node.span().start) {
            return;
        }
        let Some(matched) = legacy_memoization_pattern(first, second) else { return };

        // `offset(node)`: the column of the `begin` node (a parenthesized
        // group starts at its parenthesis).
        let start = match ctx.parent() {
            Some(parent) if parent.kind == NodeKind::ParenthesesNode => parent.span.start,
            _ => node.span().start,
        };
        let indent = " ".repeat(ctx.line_col(start).column as usize);
        let t = String::from_utf8_lossy(ctx.text(matched.t)).into_owned();
        let init = String::from_utf8_lossy(ctx.text(matched.init)).into_owned();
        let ty = String::from_utf8_lossy(ctx.text(matched.ivar_type)).into_owned();
        let ivar = &matched.ivar;
        let mut correction = format!("{ivar} ||= {t}.let({init}, {t}.nilable({ty}))");

        // We know good places to put line breaks, if required.
        let too_long = self
            .max_line_length
            .is_some_and(|max| format!("{indent}{correction}").chars().count() > max);
        if too_long || correction.contains('\n') {
            let nl = format!("\n{indent}");
            correction = format!(
                "{ivar} ||= {t}.let(\n{indent}  {},\n{indent}  {t}.nilable({}),\n{indent})",
                init.replace('\n', &nl),
                ty.replace('\n', &nl),
            );
        }

        ctx.report_with_fix(
            &Self::META,
            matched.first,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(
                    Span::new(matched.first.start, matched.second.end),
                    correction.into_bytes(),
                )],
            },
        );
    }
}

struct Matched {
    first: Span,
    second: Span,
    ivar: String,
    t: Span,
    ivar_type: Span,
    init: Span,
}

/// `(const {nil? cbase} :T)`.
fn is_t_const(node: &Node<'_>) -> bool {
    match node {
        Node::ConstantReadNode { .. } => {
            node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"T")
        }
        Node::ConstantPathNode { .. } => node.as_constant_path_node().is_some_and(|path| {
            path.parent().is_none() && path.name().is_some_and(|n| n.as_slice() == b"T")
        }),
        _ => false,
    }
}

/// A plain `send` (not `csend`, no attached block) named `name` with
/// exactly `arity` arguments.
fn plain_send<'a>(node: &'a Node<'a>, name: &[u8], arity: usize) -> Option<CallNode<'a>> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() || call.name().as_slice() != name || call.block().is_some() {
        return None;
    }
    let count = call.arguments().map_or(0, |args| args.arguments().iter().count());
    (count == arity).then_some(call)
}

/// ```text
/// (begin ...
///   $(ivasgn $_ivar
///     (send $(const {nil? cbase} :T) :let
///       (ivar _ivar)
///       (send (const {nil? cbase} :T) :nilable $_ivar_type)))
///   $(or-asgn (ivasgn _ivar) $_initialization_expr))
/// ```
fn legacy_memoization_pattern(first: &Node<'_>, second: &Node<'_>) -> Option<Matched> {
    let write = first.as_instance_variable_write_node()?;
    let ivar = write.name();
    let value = write.value();
    let let_call = plain_send(&value, b"let", 2)?;
    let t = let_call.receiver()?;
    if !is_t_const(&t) {
        return None;
    }
    let args: Vec<Node<'_>> = let_call.arguments()?.arguments().iter().collect();
    let read = args[0].as_instance_variable_read_node()?;
    if read.name().as_slice() != ivar.as_slice() {
        return None;
    }
    let nilable = plain_send(&args[1], b"nilable", 1)?;
    if !is_t_const(&nilable.receiver()?) {
        return None;
    }
    let ivar_type = nilable.arguments()?.arguments().iter().next()?;

    let or_write = second.as_instance_variable_or_write_node()?;
    if or_write.name().as_slice() != ivar.as_slice() {
        return None;
    }
    Some(Matched {
        first: first.span(),
        second: second.span(),
        ivar: String::from_utf8_lossy(ivar.as_slice()).into_owned(),
        t: t.span(),
        ivar_type: ivar_type.span(),
        init: or_write.value().span(),
    })
}
