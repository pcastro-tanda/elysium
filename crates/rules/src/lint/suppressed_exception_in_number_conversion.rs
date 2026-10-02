//! `Lint/SuppressedExceptionInNumberConversion`, ported from RuboCop's
//! `lib/rubocop/cop/lint/suppressed_exception_in_number_conversion.rb`.
//!
//! # Two shapes, one Prism node
//!
//! Upstream's `on_rescue(node)` fires on whitequark's `:rescue` node and
//! distinguishes two shapes by inspecting `node.parent`: a `(kwbegin
//! (rescue ...))` wrapper (an explicit `begin...rescue...end`, whose
//! `resbody` may restrict the rescued exception classes to
//! `ArgumentError`/`TypeError`/their `::`-qualified forms) versus a bare
//! `:rescue` with no such wrapper (a `rescue`/`rescue nil` modifier, or a
//! `def` body's implicit rescue, whose `resbody` must have no exception
//! class list at all). Prism represents an explicit `begin...end` *and* a
//! `def` body's implicit rescue with the very same [`NodeKind::BeginNode`]
//! (the trap doc's "method bodies with rescue are a `BeginNode` as
//! `def.body()` with no `begin_keyword_loc`"), and gives the modifier form
//! its own [`NodeKind::RescueModifierNode`] (see `suppressed_exception.rs`'s
//! module doc for the same split). So this rule subscribes to both kinds:
//! a `BeginNode` whose `begin_keyword_loc()` is present and has no
//! `ensure_clause()` gets the exception-class-restricted check (reporting
//! on the whole node, matching `add_offense(node.parent)` on the kwbegin);
//! any other `BeginNode` with a `rescue_clause()` (no `begin` keyword, or an
//! `ensure` sibling that means whitequark's `(kwbegin (rescue ...))` shape
//! never matched) falls back to the unrestricted check, reporting on the
//! `rescue` clause's own span (whitequark's bare `:rescue` node). A
//! `RescueModifierNode` always gets the unrestricted check.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BeginNode, CallNode, RescueNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG_TEMPLATE: (&str, &str) = ("Use `", "` instead.");

/// RuboCop's `EXPECTED_EXCEPTION_CLASSES`.
const EXPECTED_EXCEPTION_CLASSES: &[&[u8]] =
    &[b"ArgumentError", b"TypeError", b"::ArgumentError", b"::TypeError"];

/// RuboCop's `constructor_receiver?`: `{nil? (const {nil? cbase} :Kernel)}`
/// -- a receiverless call, or an explicit call on a bare or top-level-
/// qualified `Kernel` constant.
fn is_constructor_receiver(receiver: &Node<'_>) -> bool {
    match receiver.kind() {
        NodeKind::ConstantReadNode => {
            receiver.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"Kernel")
        }
        NodeKind::ConstantPathNode => receiver.as_constant_path_node().is_some_and(|path| {
            path.parent().is_none() && path.name().is_some_and(|n| n.as_slice() == b"Kernel")
        }),
        _ => false,
    }
}

/// RuboCop's `numeric_method?`: a receiverless (or `Kernel`-qualified) call
/// to `Integer`/`BigDecimal`/`Complex`/`Rational` with one or two positional
/// arguments, or to `Float` with exactly one -- and, since whitequark would
/// instead give a blocked call its own `block` node (never a `send`
/// matching this pattern), no block either.
fn numeric_method(call: &CallNode<'_>) -> bool {
    if !call.receiver().is_none_or(|r| is_constructor_receiver(&r)) {
        return false;
    }
    if call.block().is_some() {
        return false;
    }
    let arg_count = call.arguments().map_or(0, |a| a.arguments().len());
    match call.name().as_slice() {
        b"Integer" | b"BigDecimal" | b"Complex" | b"Rational" => (1..=2).contains(&arg_count),
        b"Float" => arg_count == 1,
        _ => false,
    }
}

/// RuboCop's `exception_keyword_argument?`: does any argument carry an
/// `exception:` keyword (so `Integer(arg, exception: false)` already
/// suppresses the conversion error, and adding another `exception: false`
/// would be wrong).
fn exception_keyword_argument(call: &CallNode<'_>) -> bool {
    let Some(arguments) = call.arguments() else { return false };
    arguments.arguments().iter().any(|argument| {
        let elements = match argument.kind() {
            NodeKind::KeywordHashNode => argument.as_keyword_hash_node().map(|h| h.elements()),
            NodeKind::HashNode => argument.as_hash_node().map(|h| h.elements()),
            _ => None,
        };
        elements.is_some_and(|elements| {
            elements.iter().any(|pair| {
                pair.as_assoc_node().is_some_and(|assoc| {
                    assoc.key().as_symbol_node().is_some_and(|key| key.unescaped() == b"exception")
                })
            })
        })
    })
}

/// RuboCop's `expected_exception_classes_only?`: an absent exception-class
/// list places no restriction; otherwise every listed class's own source
/// text must be one of `EXPECTED_EXCEPTION_CLASSES`.
fn expected_exception_classes_only(ctx: &Context<'_>, rescue: &RescueNode<'_>) -> bool {
    let exceptions = rescue.exceptions();
    exceptions
        .iter()
        .all(|exception| EXPECTED_EXCEPTION_CLASSES.contains(&ctx.text(exception.span())))
}

/// RuboCop's `(resbody nil? nil? (nil))`/`(resbody $_? nil? {(nil)
/// nil?})`: no `=> exc_var` reference, and a body that is either absent
/// (only valid for the kwbegin/restricted shape, via `allow_absent`) or
/// exactly a single `nil` literal.
fn resbody_body_matches(rescue: &RescueNode<'_>, allow_absent: bool) -> bool {
    if rescue.reference().is_some() {
        return false;
    }
    match rescue.statements() {
        None => allow_absent,
        Some(statements) => {
            let body = statements.body();
            body.len() == 1 && body.iter().next().is_some_and(|n| n.kind() == NodeKind::NilNode)
        }
    }
}

/// The single statement guarded by a `BeginNode`'s body, if it is exactly
/// one `numeric_method?` call -- RuboCop's `$#numeric_method?` capture
/// position, which only ever matches a lone node, never several statements.
fn single_numeric_call<'pr>(begin: &BeginNode<'pr>) -> Option<CallNode<'pr>> {
    let statements = begin.statements()?;
    let body = statements.body();
    if body.len() != 1 {
        return None;
    }
    let call = body.iter().next()?.as_call_node()?;
    numeric_method(&call).then_some(call)
}

/// RuboCop's `register_offense`'s `prefer` construction: the matched call's
/// own source, rebuilt with `exception: false` appended and its original
/// receiver/call-operator (if any) kept.
fn build_prefer(ctx: &Context<'_>, call: &CallNode<'_>) -> String {
    let name = String::from_utf8_lossy(call.name().as_slice());
    let mut arguments: Vec<String> = call
        .arguments()
        .map(|arguments| {
            arguments
                .arguments()
                .iter()
                .map(|argument| String::from_utf8_lossy(ctx.text(argument.span())).into_owned())
                .collect()
        })
        .unwrap_or_default();
    arguments.push("exception: false".to_owned());
    let base = format!("{name}({})", arguments.join(", "));
    match call.receiver() {
        Some(receiver) => {
            let receiver_source = String::from_utf8_lossy(ctx.text(receiver.span()));
            let dot_source = call
                .call_operator_loc()
                .map(|loc| String::from_utf8_lossy(ctx.text(loc.span())).into_owned())
                .unwrap_or_default();
            format!("{receiver_source}{dot_source}{base}")
        }
        None => base,
    }
}

/// Checks for cases where exceptions unrelated to the numeric constructors
/// may be unintentionally swallowed.
#[derive(Debug, Clone)]
pub struct SuppressedExceptionInNumberConversion;

impl SuppressedExceptionInNumberConversion {
    /// Shared tail of both shapes: once a numeric call and its matching
    /// `RescueNode` are known, skip a call that already has `exception:`,
    /// then report with the "add `exception: false`" fix.
    fn check(ctx: &mut Context<'_>, span: Span, call: &CallNode<'_>) {
        if exception_keyword_argument(call) {
            return;
        }
        let prefer = build_prefer(ctx, call);
        let message = format!("{}{prefer}{}", MSG_TEMPLATE.0, MSG_TEMPLATE.1);
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, prefer.into_bytes())],
            },
        );
    }

    /// RuboCop's `on_rescue`, the `BeginNode` side: both the explicit
    /// `begin...rescue...end` (kwbegin) shape and a `def` body's implicit
    /// rescue share this node kind in Prism (see the module doc).
    fn check_begin(ctx: &mut Context<'_>, node: &Node<'_>, begin: &BeginNode<'_>) {
        if begin.else_clause().is_some() {
            return;
        }
        let Some(rescue) = begin.rescue_clause() else { return };
        if rescue.subsequent().is_some() {
            return;
        }

        if begin.begin_keyword_loc().is_some() && begin.ensure_clause().is_none() {
            if !resbody_body_matches(&rescue, true)
                || !expected_exception_classes_only(ctx, &rescue)
            {
                return;
            }
            let Some(call) = single_numeric_call(begin) else { return };
            Self::check(ctx, node.span(), &call);
        } else {
            if !rescue.exceptions().is_empty() || !resbody_body_matches(&rescue, false) {
                return;
            }
            let Some(call) = single_numeric_call(begin) else { return };
            Self::check(ctx, rescue.location().span(), &call);
        }
    }

    /// RuboCop's `on_rescue`, the modifier side: `numeric_constructor_rescue_nil`
    /// applied directly, via Prism's dedicated [`NodeKind::RescueModifierNode`].
    fn check_modifier(ctx: &mut Context<'_>, node: &Node<'_>) {
        let modifier = node.as_rescue_modifier_node().expect("kind matched");
        if modifier.rescue_expression().kind() != NodeKind::NilNode {
            return;
        }
        let Some(call) = modifier.expression().as_call_node() else { return };
        if !numeric_method(&call) {
            return;
        }
        Self::check(ctx, node.span(), &call);
    }
}

impl Rule for SuppressedExceptionInNumberConversion {
    const META: RuleMeta = RuleMeta {
        name: "Lint/SuppressedExceptionInNumberConversion",
        department: Department::Lint,
        summary: "Checks for cases where exceptions unrelated to the numeric constructors may be unintentionally swallowed.",
        explanation: "\
Checks for cases where exceptions unrelated to the numeric constructors \
`Integer()`, `Float()`, `BigDecimal()`, `Complex()`, and `Rational()` may \
be unintentionally swallowed.

```ruby
# bad
Integer(arg) rescue nil

# bad
begin
  Integer(arg)
rescue
  nil
end

# good
Integer(arg, exception: false)
```

This cop's autocorrection is unsafe because unexpected errors occurring in \
the argument passed to the numeric constructor (e.g., `Integer()`) can lead \
to incompatible behavior. For example, changing `Integer(arg) rescue nil` \
to `Integer(arg, exception: false)` ensures that exceptions raised by `arg` \
itself are not ignored.",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::BeginNode, NodeKind::RescueModifierNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::BeginNode => {
                let begin = node.as_begin_node().expect("kind matched");
                Self::check_begin(ctx, node, &begin);
            }
            NodeKind::RescueModifierNode => Self::check_modifier(ctx, node),
            _ => {}
        }
    }
}
