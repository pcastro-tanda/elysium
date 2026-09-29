//! `Style/Sample`, ported from RuboCop's `lib/rubocop/cop/style/sample.rb`.
//!
//! Upstream matches `(call $(call _ :shuffle $...) ${:first :last :[] :at
//! :slice} $...)` -- a `call` node pattern matches both a plain send and a
//! safe-navigation send, so both the outer accessor and the inner `shuffle`
//! call may independently be `.` or `&.`; no `is_safe_navigation` guard is
//! needed on either.
//!
//! `sample_size` mirrors upstream's three-way `Integer` / `nil` / `:unknown`
//! result ([`SampleSize`]): a single `0` or `-1` index is a recognized
//! "whole array, no explicit size" shape (`nil`, giving a bare `sample`),
//! any other single index is unknowable without the collection's runtime
//! size (`:unknown`, not an offense), and a `[0, n]`/`0..n`-shaped range or
//! two-argument form yields a known replacement size.
//!
//! Autocorrection only fires when `shuffle` itself took no arguments --
//! `shuffle(random: ...)` consumes its generator differently than `sample`
//! would, so upstream still reports but never corrects it (see
//! `Sample#on_send`'s `shuffle_arg.empty?` branch).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{IntegerNode, RangeNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `sample_size`/`sample_size_for_one_arg`/`range_size` result:
/// a Ruby `Integer` ([`SampleSize::Known`]), `nil` for a recognized shape
/// that carries no explicit size ([`SampleSize::Nil`]), or `:unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SampleSize {
    Known(i64),
    Nil,
    Unknown,
}

/// An `IntegerNode`'s value, if it fits `i32` (as any literal small enough
/// to plausibly be a `shuffle`/`sample` index or count is).
fn int_value(node: &IntegerNode<'_>) -> Option<i64> {
    let value: i32 = node.value().try_into().ok()?;
    Some(i64::from(value))
}

/// RuboCop's `offensive?`.
fn offensive(method: &[u8], method_args: &[Node<'_>]) -> bool {
    match method {
        b"first" | b"last" => true,
        b"[]" | b"at" | b"slice" => sample_size(method_args) != SampleSize::Unknown,
        _ => false,
    }
}

/// RuboCop's `sample_size`.
fn sample_size(method_args: &[Node<'_>]) -> SampleSize {
    match method_args {
        [one] => sample_size_for_one_arg(one),
        [first, second] => sample_size_for_two_args(first, second),
        _ => SampleSize::Nil,
    }
}

/// RuboCop's `sample_size_for_one_arg`.
fn sample_size_for_one_arg(arg: &Node<'_>) -> SampleSize {
    if let Some(range) = arg.as_range_node() {
        range_size(&range)
    } else if let Some(int) = arg.as_integer_node() {
        match int_value(&int) {
            Some(0 | -1) => SampleSize::Nil,
            _ => SampleSize::Unknown,
        }
    } else {
        SampleSize::Unknown
    }
}

/// RuboCop's `sample_size_for_two_args`.
fn sample_size_for_two_args(first: &Node<'_>, second: &Node<'_>) -> SampleSize {
    let Some(first) = first.as_integer_node() else { return SampleSize::Unknown };
    if int_value(&first) != Some(0) {
        return SampleSize::Unknown;
    }
    second
        .as_integer_node()
        .and_then(|n| int_value(&n))
        .map_or(SampleSize::Unknown, SampleSize::Known)
}

/// RuboCop's `range_size`: both ends must be absent (`0`) or an integer
/// literal, the low end must be exactly `0`, and the high end must be
/// non-negative -- otherwise the range's runtime size cannot be known
/// ahead of time (a negative or non-literal endpoint, or a non-zero start).
fn range_size(range: &RangeNode<'_>) -> SampleSize {
    let side = |node: Option<Node<'_>>| match node {
        None => Some(0),
        Some(node) => node.as_integer_node().and_then(|n| int_value(&n)),
    };
    let (Some(low), Some(high)) = (side(range.left()), side(range.right())) else {
        return SampleSize::Unknown;
    };
    if low != 0 || high < 0 {
        return SampleSize::Unknown;
    }
    SampleSize::Known(if range.is_exclude_end() { high } else { high + 1 })
}

/// RuboCop's `extract_source`: the first argument's own source text, or
/// `nil` for an empty argument list.
fn extract_source(ctx: &Context<'_>, args: &[Node<'_>]) -> Option<String> {
    args.first().map(|node| String::from_utf8_lossy(ctx.text(node.span())).into_owned())
}

/// RuboCop's `sample_arg`.
fn sample_arg(ctx: &Context<'_>, method: &[u8], method_args: &[Node<'_>]) -> Option<String> {
    match method {
        b"first" | b"last" => extract_source(ctx, method_args),
        b"[]" | b"slice" => match sample_size(method_args) {
            SampleSize::Known(size) => Some(size.to_string()),
            SampleSize::Nil | SampleSize::Unknown => None,
        },
        _ => None,
    }
}

/// RuboCop's `correction`.
fn correction(
    ctx: &Context<'_>,
    method: &[u8],
    method_args: &[Node<'_>],
    shuffle_args: &[Node<'_>],
) -> String {
    let args: Vec<String> =
        [sample_arg(ctx, method, method_args), extract_source(ctx, shuffle_args)]
            .into_iter()
            .flatten()
            .collect();
    if args.is_empty() {
        "sample".to_string()
    } else {
        format!("sample({})", args.join(", "))
    }
}

/// Identifies usages of `shuffle.first`, `shuffle.last`, and `shuffle[]`
/// and change them to use `sample` instead.
#[derive(Debug, Clone)]
pub struct Sample;

impl Rule for Sample {
    const META: RuleMeta = RuleMeta {
        name: "Style/Sample",
        department: Department::Style,
        summary: "Use `sample` instead of `shuffle.first`, `shuffle.last`, and `shuffle[Integer]`.",
        explanation: "",
        enabled_by_default: true,
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
        let method = call.name();
        let method = method.as_slice();
        if !matches!(method, b"first" | b"last" | b"[]" | b"at" | b"slice") {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(shuffle_call) = receiver.as_call_node() else { return };
        if shuffle_call.name().as_slice() != b"shuffle" {
            return;
        }
        let Some(message_loc) = shuffle_call.message_loc() else { return };

        let method_args: Vec<Node<'_>> =
            call.arguments().map_or_else(Vec::new, |args| args.arguments().iter().collect());
        if !offensive(method, &method_args) {
            return;
        }

        let shuffle_args: Vec<Node<'_>> = shuffle_call
            .arguments()
            .map_or_else(Vec::new, |args| args.arguments().iter().collect());

        let range = Span::new(message_loc.span().start, node.span().end);
        let correct = correction(ctx, method, &method_args, &shuffle_args);
        let incorrect = String::from_utf8_lossy(ctx.text(range)).into_owned();
        let message = format!("Use `{correct}` instead of `{incorrect}`.");

        if shuffle_args.is_empty() {
            let fix = Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, correct.into_bytes())],
            };
            ctx.report_with_fix(&Self::META, range, message, fix);
        } else {
            ctx.report(&Self::META, range, message);
        }
    }
}
