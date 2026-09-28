//! `Lint/AmbiguousOperator`, ported from RuboCop's
//! `lib/rubocop/cop/lint/ambiguous_operator.rb`.
//!
//! # Prism shape
//!
//! Upstream drives this cop entirely off `processed_source.diagnostics`
//! (whitequark parser reason `:ambiguous_prefix`), then re-locates an AST
//! node at the diagnostic's position purely to build the autocorrection and
//! (for the unary `+`/`-` case) to exclude safe navigation. Prism already
//! reports the same ambiguity as parser warnings, in two message shapes:
//! `` ambiguous `X` has been interpreted as an argument prefix `` for a
//! splat/double-splat/block-pass argument (`SplatNode`, `AssocSplatNode`,
//! `BlockArgumentNode`), and `ambiguous first argument; put parentheses or a
//! space even after `X` operator` for a bare unary `+`/`-` first argument.
//! [`Context::parsed`]`.warnings()` surfaces both verbatim, so this port
//! parses the operator back out of the warning text instead of
//! reimplementing the parser's own ambiguity detection.
//!
//! For the unary case, upstream's second matcher (`ast.each_node(:send)`)
//! only walks *regular* `send` nodes, never `csend`; a `CallNode` with
//! `is_safe_navigation()` is therefore never found and the diagnostic is
//! silently dropped (see `do_something&.* -1`, which Prism still warns
//! about). This port mirrors that by excluding safe-navigation calls from
//! the unary search. The splat/block-pass/double-splat search
//! (`ast.each_node(:splat, :block_pass, :kwsplat)`) has no such
//! restriction, so it matches under safe navigation too.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _};
use ruby_source::Span;

/// RuboCop's `AMBIGUITIES` table.
fn ambiguity(operator: &str) -> Option<(&'static str, &'static str)> {
    Some(match operator {
        "+" => ("positive number", "an addition"),
        "-" => ("negative number", "a subtraction"),
        "*" => ("splat", "a multiplication"),
        "&" => ("block", "a binary AND"),
        "**" => ("keyword splat", "an exponent"),
        _ => return None,
    })
}

/// RuboCop's `MSG_FORMAT`.
fn message(operator: &str) -> Option<String> {
    let (actual, possible) = ambiguity(operator)?;
    Some(format!(
        "Ambiguous {actual} operator. Parenthesize the method arguments if it's surely a \
         {actual} operator, or add a whitespace to the right of the `{operator}` if it should \
         be {possible}."
    ))
}

/// Extracts `X` from Prism's `` ambiguous `X` has been interpreted as an
/// argument prefix `` warning.
fn prefix_operator(msg: &str) -> Option<&str> {
    let rest = msg.strip_prefix("ambiguous `")?;
    let (op, rest) = rest.split_once('`')?;
    (rest == " has been interpreted as an argument prefix").then_some(op)
}

/// Extracts `X` from Prism's `ambiguous first argument; put parentheses or
/// a space even after `X` operator` warning.
fn unary_operator(msg: &str) -> Option<&str> {
    let rest =
        msg.strip_prefix("ambiguous first argument; put parentheses or a space even after `")?;
    let (op, rest) = rest.split_once('`')?;
    (rest == " operator").then_some(op)
}

/// RuboCop's `find_offense_node_by` for a `:splat`/`:block_pass`/`:kwsplat`
/// diagnostic: any call whose splat, double-splat, or block-pass argument
/// begins at `span`.
fn find_prefix_call<'pr>(root: &Node<'pr>, span: Span) -> Option<CallNode<'pr>> {
    let mut found = None;
    each_descendant(root, &mut |node| {
        if found.is_some() {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if let Some(block_arg) = call.block().and_then(|b| b.as_block_argument_node()) {
            if block_arg.operator_loc().span().start == span.start {
                found = Some(call);
                return;
            }
        }
        let Some(args) = call.arguments() else { return };
        for arg in &args.arguments() {
            if let Some(splat) = arg.as_splat_node() {
                if splat.operator_loc().span().start == span.start {
                    found = Some(call);
                    return;
                }
            } else if let Some(assoc_splat) = arg.as_assoc_splat_node() {
                if assoc_splat.operator_loc().span().start == span.start {
                    found = Some(call);
                    return;
                }
            } else if let Some(hash) = arg.as_keyword_hash_node() {
                for element in &hash.elements() {
                    if let Some(assoc_splat) = element.as_assoc_splat_node() {
                        if assoc_splat.operator_loc().span().start == span.start {
                            found = Some(call);
                            return;
                        }
                    }
                }
            }
        }
    });
    found
}

/// RuboCop's `find_offense_node_by` for the unary `+`/`-` case: a
/// non-safe-navigation call whose first argument begins at `span`.
fn find_unary_call<'pr>(root: &Node<'pr>, span: Span) -> Option<CallNode<'pr>> {
    let mut found = None;
    each_descendant(root, &mut |node| {
        if found.is_some() {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() {
            return;
        }
        let Some(first) = call.arguments().and_then(|a| a.arguments().first()) else { return };
        if first.span().start == span.start {
            found = Some(call);
        }
    });
    found
}

/// RuboCop's `Util#add_parentheses` for a plain `send` node: the single gap
/// character right after the method name becomes `(`, and `)` is inserted
/// right after the call's own span (which, unparenthesized, ends at the
/// last argument or block-pass).
fn build_fix(call: &CallNode<'_>) -> Option<Fix> {
    let gap_start = call.message_loc()?.span().end;
    let close_at = call.as_node().span().end;
    Some(Fix {
        applicability: Applicability::Safe,
        edits: vec![
            Edit::replace(Span::new(gap_start, gap_start + 1), b"(".to_vec()),
            Edit::insert(close_at, b")".to_vec()),
        ],
    })
}

/// Checks for ambiguous operators in the first argument of a method invocation without parentheses.
#[derive(Debug, Clone)]
pub struct AmbiguousOperator;

impl Rule for AmbiguousOperator {
    const META: RuleMeta = RuleMeta {
        name: "Lint/AmbiguousOperator",
        department: Department::Lint,
        summary: "Checks for ambiguous operators in the first argument of a method invocation \
                   without parentheses.",
        explanation: "\
```ruby
# bad

# The `*` is interpreted as a splat operator but it could possibly be
# a `*` method invocation (i.e. `do_something.*(some_array)`).
do_something *some_array

# good

# With parentheses, there's no ambiguity.
do_something(*some_array)
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "\
Relies entirely on Prism's own parser warnings for ambiguity detection rather than \
reimplementing RuboCop's whitequark-based diagnostic logic; any case where Prism's parser \
disagrees with whitequark's about what counts as an ambiguous prefix is not reproduced. \
`self.autocorrect_incompatible_with` (`Naming::BlockForwarding`) is not ported: this port has \
no cross-rule autocorrect-conflict mechanism.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let warnings: Vec<_> = ctx.parsed().warnings().collect();
        for warning in warnings {
            if let Some(operator) = prefix_operator(&warning.message) {
                let Some(msg) = message(operator) else { continue };
                let Some(call) = find_prefix_call(&root, warning.span) else { continue };
                match build_fix(&call) {
                    Some(fix) => ctx.report_with_fix(&Self::META, warning.span, msg, fix),
                    None => ctx.report(&Self::META, warning.span, msg),
                }
            } else if let Some(operator) = unary_operator(&warning.message) {
                let Some(msg) = message(operator) else { continue };
                let Some(call) = find_unary_call(&root, warning.span) else { continue };
                match build_fix(&call) {
                    Some(fix) => ctx.report_with_fix(&Self::META, warning.span, msg, fix),
                    None => ctx.report(&Self::META, warning.span, msg),
                }
            }
        }
    }
}
