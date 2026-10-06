//! `Performance/CaseWhenSplat`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/case_when_splat.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_ast::node::{CaseNode, WhenNode};
use ruby_source::Span;

const MSG: &str = "Reordering `when` conditions with a splat to the end of the `when` branches can improve performance.";
const ARRAY_MSG: &str = "Pass the contents of array literals directly to `when` conditions.";

/// Reordering `when` conditions with a splat to the end of the `when` branches can improve performance.
#[derive(Debug, Clone)]
pub struct CaseWhenSplat;

impl Rule for CaseWhenSplat {
    const META: RuleMeta = RuleMeta {
        name: "Performance/CaseWhenSplat",
        department: Department::Performance,
        summary: "Reordering `when` conditions with a splat to the end of the `when` branches can improve performance.",
        explanation: "\
Ruby has to allocate memory for the splat expansion every time that the
`case` `when` statement is run. Since Ruby does not support fall through
inside of `case` `when`, like some other languages do, the order of the
`when` branches should not matter. By placing any splat expansions at the
end of the list of `when` branches we will reduce the number of times that
memory has to be allocated for the expansion. The exception to this is if
multiple of your `when` conditions can be true for any given condition. A
likely scenario for this defining a higher level when condition to override
a condition that is inside of the splat expansion.

@safety
This cop is not unsafe autocorrection because it is not a guaranteed
performance improvement. If the data being processed by the `case`
condition is normalized in a manner that favors hitting a condition in the
splat expansion, it is possible that moving the splat condition to the end
will use more memory, and run slightly slower.

```ruby
# bad
case foo
when *condition
  bar
when baz
  foobar
end

case foo
when *[1, 2, 3, 4]
  bar
when 5
  baz
end

# good
case foo
when baz
  foobar
when *condition
  bar
end

case foo
when 1, 2, 3, 4
  bar
when 5
  baz
end
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CaseNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(case_node) = node.as_case_node() else { return };
        let mut whens: Vec<WhenNode<'_>> = Vec::new();
        for branch in &case_node.conditions() {
            let Some(when) = branch.as_when_node() else { return };
            whens.push(when);
        }
        let conditions: Vec<(usize, Node<'_>)> = whens
            .iter()
            .enumerate()
            .flat_map(|(i, w)| w.conditions().iter().map(move |c| (i, c)))
            .collect();

        let mut found_non_splat = false;
        let mut offenses: Vec<usize> = Vec::new();
        for (idx, (_, condition)) in conditions.iter().enumerate().rev() {
            let non_splat = is_non_splat(condition);
            found_non_splat |= non_splat;
            if !non_splat && found_non_splat {
                offenses.push(idx);
            }
        }

        let mut ignored = vec![false; whens.len()];
        for idx in offenses.into_iter().rev() {
            let (when_idx, condition) = &conditions[idx];
            if ignored[*when_idx] {
                continue;
            }
            ignored[*when_idx] = true;
            let when = &whens[*when_idx];
            let variable_is_array = condition
                .as_splat_node()
                .and_then(|s| s.expression())
                .is_some_and(|e| e.as_array_node().is_some());
            let message = if variable_is_array { ARRAY_MSG } else { MSG };
            let span = Span::new(when.keyword_loc().span().start, condition.span().end);
            match autocorrect(ctx, &case_node, &whens, *when_idx) {
                Some(edits) => ctx.report_with_fix(
                    &Self::META,
                    span,
                    message,
                    Fix { applicability: Applicability::Unsafe, edits },
                ),
                None => ctx.report(&Self::META, span, message),
            }
        }
    }
}

fn is_non_splat(condition: &Node<'_>) -> bool {
    match condition.as_splat_node() {
        Some(splat) => splat.expression().is_some_and(|e| e.as_array_node().is_some()),
        None => true,
    }
}

/// Whitequark's `when` expression end: the body's end, else the last
/// condition's end (the `then` keyword is not part of it).
fn when_end(when: &WhenNode<'_>) -> u32 {
    match when.statements() {
        Some(stmts) => stmts.location().span().end,
        None => when.conditions().iter().last().map_or(when.keyword_loc().span().end, |c| c.span().end),
    }
}

fn replacement(ctx: &Context<'_>, conditions: &[Node<'_>]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut first = true;
    for splat_group in [false, true] {
        for c in conditions.iter().filter(|c| c.as_splat_node().is_some() == splat_group) {
            if !first {
                out.extend_from_slice(b", ");
            }
            first = false;
            out.extend_from_slice(ctx.text(c.span()));
        }
    }
    out
}

fn autocorrect(
    ctx: &Context<'_>,
    case_node: &CaseNode<'_>,
    whens: &[WhenNode<'_>],
    idx: usize,
) -> Option<Vec<Edit>> {
    let when = &whens[idx];
    let conditions: Vec<Node<'_>> = when.conditions().iter().collect();
    let needs_reorder = whens[idx + 1..]
        .iter()
        .any(|w| w.conditions().iter().any(|c| is_non_splat(&c)));

    if !needs_reorder {
        let first = conditions.first()?.span().start;
        let last = conditions.last()?.span().end;
        return Some(vec![Edit::replace(Span::new(first, last), replacement(ctx, &conditions))]);
    }
    if whens.len() == 1 {
        return None;
    }

    let when_start = when.keyword_loc().span().start;
    let next_start = whens[idx + 1].keyword_loc().span().start;
    let new_condition = replacement(ctx, &conditions);
    let indent = " ".repeat(ctx.line_col(when_start).column as usize);
    let body = when.statements().map(|s| s.location().span());

    let mut text: Vec<u8> = Vec::new();
    let same_line = body
        .is_some_and(|b| ctx.line_col(when_start).line == ctx.line_col(b.start).line);
    if same_line {
        let body = body?;
        text.extend_from_slice(format!("\n{indent}when ").as_bytes());
        text.extend_from_slice(&new_condition);
        text.extend_from_slice(b" then ");
        text.extend_from_slice(ctx.text(body));
    } else {
        text.extend_from_slice(format!("\n{indent}when ").as_bytes());
        text.extend_from_slice(&new_condition);
        text.push(b'\n');
        if let Some(body) = body {
            text.extend_from_slice(" ".repeat(ctx.line_col(body.start).column as usize).as_bytes());
            text.extend_from_slice(ctx.text(body));
        }
    }

    let start_line = ctx.line_col(when_start).line;
    let end_line = find_end_line(ctx, case_node, whens, idx);
    let comments: Vec<Vec<u8>> = ctx
        .comments()
        .iter()
        .filter(|c| c.line >= start_line && c.line < end_line)
        .map(|c| {
            let mut s = " ".repeat(ctx.line_col(c.span.start).column as usize).into_bytes();
            s.extend_from_slice(ctx.text(c.span));
            s
        })
        .collect();
    text.extend_from_slice(&comments.join(&b'\n'));

    let last_end = when_end(whens.last()?);
    Some(vec![
        Edit::delete(Span::new(when_start, next_start)),
        Edit::insert(last_end, text),
    ])
}

/// `CommentsHelp#find_end_line` for a `when` node.
fn find_end_line(
    ctx: &Context<'_>,
    case_node: &CaseNode<'_>,
    whens: &[WhenNode<'_>],
    idx: usize,
) -> u32 {
    if let Some(next) = whens.get(idx + 1) {
        return ctx.line_col(next.keyword_loc().span().start).line;
    }
    if let Some(stmts) = case_node.else_clause().and_then(|e| e.statements()) {
        return ctx.line_col(stmts.location().span().start).line;
    }
    ctx.line_col(case_node.end_keyword_loc().span().start).line
}
