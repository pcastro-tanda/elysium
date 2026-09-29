//! `Style/TrailingUnderscoreVariable`, ported from RuboCop's
//! `lib/rubocop/cop/style/trailing_underscore_variable.rb`.
//!
//! # Node shapes
//!
//! Whitequark's top-level `masgn` node has a separate `mlhs` child holding
//! just the targets; Prism instead folds the targets, operator and value
//! straight into one [`NodeKind::MultiWriteNode`] (`lefts`/`rest`/`rights`
//! plus its own optional `lparen_loc`/`rparen_loc` for a fully-parenthesized
//! LHS, `operator_loc`, `value`). A *nested* destructuring group (`(b, _)`
//! inside `a, (b, _) = ...`) is instead a bare [`NodeKind::MultiTargetNode`]
//! -- the direct analogue of whitequark's `mlhs` -- with the same
//! `lefts`/`rest`/`rights`/`lparen_loc`/`rparen_loc` fields but no operator
//! or value, since it never has a right-hand side of its own. This port
//! flattens both shapes into a plain `Vec<Node>` of targets plus a few
//! pieces of shared context ([`collect_ranges`]'s parameters) so the same
//! recursive algorithm handles the top `on_masgn` call and every nested
//! group upstream reaches through `unneeded_ranges`' `node.masgn_type? ?
//! node.lhs : node` branch.
//!
//! # Which targets count
//!
//! Upstream's `DISALLOW = %i[lvasgn splat]` restricts the trailing-run scan
//! to plain local variable targets and splats -- an instance/class/global
//! variable, constant, attribute/index or nested-group target always stops
//! the scan (its `Node#type` lookup misses `DISALLOW` before its name is
//! even inspected). Since none of those other target kinds can ever have a
//! name starting with `_` in a way that matters here (an ivar's own name
//! starts with `@`, a gvar's with `$`, ...), [`qualifies`] collapses both
//! the type check and the name check into one predicate over
//! [`NodeKind::LocalVariableTargetNode`] and [`NodeKind::SplatNode`]
//! (recursing into the splat's own `expression`, when present, the same way
//! upstream's `var, = *variable; var, = *var` double-splat unwraps a
//! `(splat (lvasgn :_c))` down to the bare symbol `:_c`) without changing
//! behaviour for any other kind.
//!
//! # Message text and diagnostic order
//!
//! Every offense's message embeds `node.source` (the *entire* top-level
//! `masgn` statement, LHS, operator and RHS alike -- confirmed by every
//! fixture's `Prefer` text including the ` = ...` tail) with just that one
//! offense's own range excised, computed once per report from the absolute
//! byte offsets so it stays correct regardless of nesting depth. Upstream
//! builds its `ranges` array as `children_offenses(variables) <<
//! main_offense` (children first, the enclosing group's own offense last);
//! this port instead reports the enclosing group's offense before
//! recursing into its nested groups, since the fixture harness sorts every
//! file's annotations by `(line, message, column)` before comparing and
//! diagnostic order is otherwise unobserved.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG_PREFIX: &str = "Do not use trailing `_`s in parallel assignment. Prefer `";

/// Checks for the usage of unneeded trailing underscores at the end of parallel variable assignment.
#[derive(Debug, Clone)]
pub struct TrailingUnderscoreVariable {
    allow_named_underscore_variables: bool,
}

/// `variable`'s underscore-target name, if it is one of the two kinds
/// [`qualifies`] cares about: a bare local variable target, or a splat
/// wrapping one (`*_`/`*_foo`). `None` for a bare `*` (no `expression`) and
/// every other target kind.
fn underscore_target_name<'pr>(variable: &Node<'pr>) -> Option<&'pr [u8]> {
    if let Some(target) = variable.as_local_variable_target_node() {
        return Some(target.name().as_slice());
    }
    if let Some(splat) = variable.as_splat_node() {
        let expr = splat.expression()?;
        return Some(expr.as_local_variable_target_node()?.name().as_slice());
    }
    None
}

/// Whether `variable` may extend a trailing underscore run: RuboCop's
/// combined `DISALLOW.include?(variable.type)` type check and
/// `allow_named_underscore_variables && var != :_` / `!var.to_s
/// .start_with?(UNDERSCORE)` name check.
fn qualifies(variable: &Node<'_>, allow_named_underscore_variables: bool) -> bool {
    let Some(name) = underscore_target_name(variable) else { return false };
    if allow_named_underscore_variables {
        name == b"_"
    } else {
        name.starts_with(b"_")
    }
}

/// RuboCop's `find_first_offense`/`find_first_possible_offense`: the index
/// of the leftmost variable in the maximal trailing run of qualifying
/// variables, or `None` when the very last variable does not qualify.
fn find_first_offense_idx(
    variables: &[Node<'_>],
    allow_named_underscore_variables: bool,
) -> Option<usize> {
    let mut found = None;
    for i in (0..variables.len()).rev() {
        if qualifies(&variables[i], allow_named_underscore_variables) {
            found = Some(i);
        } else {
            break;
        }
    }
    found
}

/// RuboCop's `splat_variable_before?`: any variable strictly before `idx`
/// (regardless of whether it itself qualifies) is a splat. `*a, b, _` would
/// otherwise remove two variables from `a` with no way to say so, and
/// `a, *b, _` would need to rewrite `*b, =` into a syntax error.
fn splat_before(variables: &[Node<'_>], idx: usize) -> bool {
    variables[..idx].iter().any(|v| v.as_splat_node().is_some())
}

/// RuboCop's `Util.parentheses?(mlhs_node)` applied through this port's
/// flattened representation: the byte range of the target list itself
/// (parens included when present), used both for `unused_range`'s "delete
/// the whole group" end and `range_for_parentheses`'s trailing boundary.
fn group_span(variables: &[Node<'_>], lparen: Option<Span>, rparen: Option<Span>) -> Span {
    match (lparen, rparen) {
        (Some(l), Some(r)) => Span::new(l.start, r.end),
        _ => Span::new(variables[0].span().start, variables[variables.len() - 1].span().end),
    }
}

/// `lefts`/`rest`/`rights`, flattened into unpacking order -- RuboCop-AST's
/// `variables = *mlhs_node`.
fn flatten_targets<'pr>(
    lefts: impl Iterator<Item = Node<'pr>>,
    rest: Option<Node<'pr>>,
    rights: impl Iterator<Item = Node<'pr>>,
) -> Vec<Node<'pr>> {
    let mut vars: Vec<Node<'pr>> = lefts.collect();
    // A bare trailing comma with nothing after it (`a, = foo`, `(a, b,) = foo`)
    // parses as an `ImplicitRestNode` placeholder marking where the comma is,
    // not a real target -- RuboCop-AST's `variables = *mlhs_node` has no such
    // placeholder (whitequark just omits it), so it must not join the scan.
    if let Some(rest) = rest {
        if rest.kind() != NodeKind::ImplicitRestNode {
            vars.push(rest);
        }
    }
    vars.extend(rights);
    vars
}

/// The two `masgn`-only offsets `collect_ranges` needs for `unused_range`'s
/// masgn branch and `range_between(first_offense_begin, node.loc.operator
/// .begin_pos)`. `None` for a nested group's own recursive call, which has
/// neither an operator nor a right-hand side.
struct MasgnEnds {
    operator_start: u32,
    rhs_start: u32,
}

/// RuboCop's `unneeded_ranges`/`main_node_offense`/`unused_range`/
/// `range_for_parentheses`, applied to one target list (the top `masgn`'s
/// own, or a nested group's). `masgn` selects `unused_range`'s two branches
/// (the whole group vanishes into the RHS start vs. into the group's own
/// end) and supplies the trailing boundary for the unparenthesized case.
/// Recurses into every nested [`MultiTargetNode`] target regardless of
/// where the trailing run landed, matching `children_offenses(variables) =
/// variables.select(&:mlhs_type?)`.
fn collect_ranges(
    variables: &[Node<'_>],
    masgn: Option<MasgnEnds>,
    lparen: Option<Span>,
    rparen: Option<Span>,
    allow_named_underscore_variables: bool,
    ranges: &mut Vec<Span>,
) {
    if !variables.is_empty() {
        if let Some(idx) = find_first_offense_idx(variables, allow_named_underscore_variables) {
            if !splat_before(variables, idx) {
                let g_span = group_span(variables, lparen, rparen);
                if idx == 0 {
                    let end = masgn.map_or(g_span.end, |m| m.rhs_start);
                    ranges.push(Span::new(g_span.start, end));
                } else {
                    let offense_span = variables[idx].span();
                    if lparen.is_some() {
                        ranges.push(Span::new(offense_span.start - 1, g_span.end - 1));
                    } else {
                        let operator_start = masgn.map_or(g_span.end, |m| m.operator_start);
                        ranges.push(Span::new(offense_span.start, operator_start));
                    }
                }
            }
        }
    }

    for variable in variables {
        if let Some(group) = variable.as_multi_target_node() {
            let child_vars =
                flatten_targets(group.lefts().iter(), group.rest(), group.rights().iter());
            let l = group.lparen_loc().map(|loc| loc.span());
            let r = group.rparen_loc().map(|loc| loc.span());
            collect_ranges(&child_vars, None, l, r, allow_named_underscore_variables, ranges);
        }
    }
}

impl Rule for TrailingUnderscoreVariable {
    const META: RuleMeta = RuleMeta {
        name: "Style/TrailingUnderscoreVariable",
        department: Department::Style,
        summary: "Checks for the usage of unneeded trailing underscores at the end of parallel variable assignment.",
        explanation: "\
Checks for extra underscores in variable assignment.

```ruby
# bad
a, b, _ = foo
a, b, _, = foo
a, _, _ = foo
a, _, _, = foo

# good
a, b, = foo
a, = foo
*a, b, _ = foo
# => We need to know to not include 2 variables in a
a, *b, _ = foo
# => The correction `a, *b, = foo` is a syntax error
```

With `AllowNamedUnderscoreVariables: true` (default):

```ruby
# good
a, b, _something = foo
```

With `AllowNamedUnderscoreVariables: false`:

```ruby
# bad
a, b, _something = foo
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::MultiWriteNode],
        config: &[ConfigOption {
            name: "AllowNamedUnderscoreVariables",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Whether a splat/local variable target named with a leading underscore other \
                  than a bare `_` (e.g. `_foo`) also counts as a trailing underscore variable.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_named_underscore_variables: options.bool("AllowNamedUnderscoreVariables") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(masgn) = node.as_multi_write_node() else { return };
        let variables = flatten_targets(masgn.lefts().iter(), masgn.rest(), masgn.rights().iter());
        if variables.is_empty() {
            return;
        }

        let lparen = masgn.lparen_loc().map(|l| l.span());
        let rparen = masgn.rparen_loc().map(|l| l.span());
        let operator_start = masgn.operator_loc().span().start;
        let rhs_start = masgn.value().span().start;

        let mut ranges: Vec<Span> = Vec::new();
        collect_ranges(
            &variables,
            Some(MasgnEnds { operator_start, rhs_start }),
            lparen,
            rparen,
            self.allow_named_underscore_variables,
            &mut ranges,
        );
        if ranges.is_empty() {
            return;
        }

        let node_span = masgn.as_node().span();
        let node_text = ctx.text(node_span);
        for range in ranges {
            let start_off = (range.start - node_span.start) as usize;
            let end_off = (range.end - node_span.start) as usize;
            let mut good_code = node_text.to_vec();
            good_code.drain(start_off..end_off);
            let code = String::from_utf8_lossy(&good_code);
            let message = format!("{MSG_PREFIX}{code}`.");
            let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(range)] };
            ctx.report_with_fix(&Self::META, range, message, fix);
        }
    }
}
