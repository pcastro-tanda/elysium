//! `Style/SoleNestedConditional`, ported from RuboCop's
//! `lib/rubocop/cop/style/sole_nested_conditional.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{walk, LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::{Side, Span};

/// RuboCop's `MSG`.
fn message(conditional_type: &str) -> String {
    format!("Consider merging nested conditions into outer `{conditional_type}` conditions.")
}

/// Finds a sole nested conditional that can be merged into its outer
/// `if`/`unless`.
#[derive(Debug, Clone)]
pub struct SoleNestedConditional {
    allow_modifier: bool,
}

impl Rule for SoleNestedConditional {
    const META: RuleMeta = RuleMeta {
        name: "Style/SoleNestedConditional",
        department: Department::Style,
        summary: "Finds sole nested conditional nodes which can be merged into outer \
                  conditional node.",
        explanation: "\
If the branch of a conditional consists solely of a conditional node, its
conditions can be combined with the conditions of the outer branch. This
helps to keep the nesting level from getting too deep.

```ruby
# bad
if condition_a
  if condition_b
    do_something
  end
end

# bad
if condition_b
  do_something
end if condition_a

# good
if condition_a && condition_b
  do_something
end
```

With `AllowModifier: false` (the default):

```ruby
# bad
if condition_a
  do_something if condition_b
end
```

With `AllowModifier: true`:

```ruby
# good
if condition_a
  do_something if condition_b
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode],
        config: &[ConfigOption {
            name: "AllowModifier",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Allow modifier-form `if`/`unless` to be nested without merging.",
        }],
        blind_spots: "\
`use_variable_assignment_in_condition?` (the guard against merging an outer variable
assignment into a nested modifier that reads that same variable) only recognizes plain
local-variable targets (`var = ...`, `var &&= ...`, `var ||= ...`, `var op= ...`);
instance/class/global-variable and constant assignments in the outer condition are not
tracked, so an extremely rare `if @x = foo\n  do_something if @x\nend` would be flagged
where real RuboCop stays silent.
Assignment detection for a plain (single `=`) attribute or index writer (`obj.attr =
val`, `arr[i] = val`) is approximated via Prism's `equal_loc` presence on a `CallNode`
rather than RuboCop's exact `setter_method?` check.
`self.autocorrect_incompatible_with` (avoiding a double-correction clash with
`Style/NegatedIf`/`Style/NegatedUnless` when both cops run together) is not
implemented; each cop computes its fix independently.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_modifier: options.bool("AllowModifier") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::IfNode { .. } | Node::UnlessNode { .. } => self.handle(node, ctx),
            _ => {}
        }
    }
}

impl SoleNestedConditional {
    /// RuboCop's `on_if`.
    fn handle(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(outer) = shape_of(node) else { return };
        if outer.has_else || outer.is_elsif {
            return;
        }
        let Branch::Single(inner_node) = &outer.if_branch else { return };
        if !matches!(inner_node, Node::IfNode { .. } | Node::UnlessNode { .. }) {
            return;
        }
        let Some(inner) = shape_of(inner_node) else { return };
        if inner.has_else {
            return;
        }
        if use_variable_assignment_in_condition(&outer.predicate, inner.predicate.span(), ctx) {
            return;
        }
        let outer_modifier = outer.end_span.is_none();
        let inner_modifier = inner.end_span.is_none();
        if (outer_modifier || inner_modifier) && self.allow_modifier {
            return;
        }

        let conditional_type = if outer.is_unless { "unless" } else { "if" };
        let fix = build_fix(&outer, &inner, ctx);
        ctx.report_with_fix(&Self::META, inner.keyword_span, message(conditional_type), fix);
    }
}

/// Which branch (of a plain `if`/`unless`, no `else`) holds the body.
/// Mirrors `rubocop-ast`'s `IfNode#if_branch`: a single statement is
/// unwrapped, an empty body is `Empty`, multiple statements report `Multi`
/// (never itself a candidate for merging).
enum Branch<'pr> {
    Empty,
    Single(Node<'pr>),
    Multi,
}

fn branch_of(statements: Option<ruby_ast::node::StatementsNode<'_>>) -> Branch<'_> {
    let Some(statements) = statements else { return Branch::Empty };
    let mut items = statements.body().iter();
    let Some(first) = items.next() else { return Branch::Empty };
    if items.next().is_some() {
        Branch::Multi
    } else {
        Branch::Single(first)
    }
}

/// A normalized view of one `if`/`unless` node, spanning both Prism node
/// kinds (RuboCop's whitequark-based AST unifies them, Prism does not).
struct Shape<'pr> {
    is_unless: bool,
    /// True when this node is itself an `elsif` continuation.
    is_elsif: bool,
    keyword_span: Span,
    predicate: Node<'pr>,
    /// `None` for a modifier-form conditional (no explicit `end`).
    end_span: Option<Span>,
    has_else: bool,
    if_branch: Branch<'pr>,
}

/// Builds a [`Shape`] for an `IfNode`/`UnlessNode`. Returns `None` for a
/// ternary (`a ? b : c`, also an `IfNode` in Prism with no `if`/`elsif`
/// keyword), matching RuboCop's `node.ternary?` short-circuit in `on_if`.
fn shape_of<'pr>(node: &Node<'pr>) -> Option<Shape<'pr>> {
    match node {
        Node::IfNode { .. } => {
            let n = node.as_if_node().expect("kind matched");
            let kw_loc = n.if_keyword_loc()?;
            Some(Shape {
                is_unless: false,
                is_elsif: kw_loc.as_slice() == b"elsif",
                keyword_span: kw_loc.span(),
                predicate: n.predicate(),
                end_span: n.end_keyword_loc().map(|l| l.span()),
                has_else: n.subsequent().is_some(),
                if_branch: branch_of(n.statements()),
            })
        }
        Node::UnlessNode { .. } => {
            let n = node.as_unless_node().expect("kind matched");
            Some(Shape {
                is_unless: true,
                is_elsif: false,
                keyword_span: n.keyword_loc().span(),
                predicate: n.predicate(),
                end_span: n.end_keyword_loc().map(|l| l.span()),
                has_else: n.else_clause().is_some(),
                if_branch: branch_of(n.statements()),
            })
        }
        _ => None,
    }
}

/// RuboCop's `use_variable_assignment_in_condition?`: skips a nested
/// modifier whose condition reads a local variable that the outer
/// condition just assigned (the modifier depends on the assignment's
/// *side effect*, so merging the conditions would change behaviour).
fn use_variable_assignment_in_condition(
    condition: &Node<'_>,
    inner_predicate_span: Span,
    ctx: &Context<'_>,
) -> bool {
    struct Finder<'a> {
        names: &'a mut Vec<Vec<u8>>,
    }
    impl<'pr> Visitor<'pr> for Finder<'_> {
        fn enter(&mut self, node: &Node<'pr>) {
            if let Some(name) = local_var_write_name(node) {
                self.names.push(name.to_vec());
            }
        }
    }
    let mut names = Vec::new();
    walk(condition, &mut Finder { names: &mut names });
    if names.is_empty() {
        return false;
    }
    let inner_text = ctx.text(inner_predicate_span);
    names.iter().any(|name| name.as_slice() == inner_text)
}

/// The assigned variable's name for the four local-variable write kinds
/// (plain `=` and the `&&=`/`||=`/`op=` shorthands).
fn local_var_write_name<'pr>(node: &Node<'pr>) -> Option<&'pr [u8]> {
    match node {
        Node::LocalVariableWriteNode { .. } => {
            Some(node.as_local_variable_write_node().expect("kind matched").name().as_slice())
        }
        Node::LocalVariableAndWriteNode { .. } => {
            Some(node.as_local_variable_and_write_node().expect("kind matched").name().as_slice())
        }
        Node::LocalVariableOrWriteNode { .. } => {
            Some(node.as_local_variable_or_write_node().expect("kind matched").name().as_slice())
        }
        Node::LocalVariableOperatorWriteNode { .. } => Some(
            node.as_local_variable_operator_write_node().expect("kind matched").name().as_slice(),
        ),
        _ => None,
    }
}

/// RuboCop's `Node#assignment?` for the assignable node kinds this cop
/// cares about: the plain-`=` and shorthand (`&&=`/`||=`/`op=`) writers for
/// locals, ivars, cvars, gvars, constants and constant paths; compound
/// attribute/index writers; multiple assignment; and a plain attribute or
/// index writer approximated via `CallNode::equal_loc`.
fn is_assignment_kind(node: &Node<'_>) -> bool {
    match node {
        Node::LocalVariableWriteNode { .. }
        | Node::LocalVariableAndWriteNode { .. }
        | Node::LocalVariableOrWriteNode { .. }
        | Node::LocalVariableOperatorWriteNode { .. }
        | Node::InstanceVariableWriteNode { .. }
        | Node::InstanceVariableAndWriteNode { .. }
        | Node::InstanceVariableOrWriteNode { .. }
        | Node::InstanceVariableOperatorWriteNode { .. }
        | Node::ClassVariableWriteNode { .. }
        | Node::ClassVariableAndWriteNode { .. }
        | Node::ClassVariableOrWriteNode { .. }
        | Node::ClassVariableOperatorWriteNode { .. }
        | Node::GlobalVariableWriteNode { .. }
        | Node::GlobalVariableAndWriteNode { .. }
        | Node::GlobalVariableOrWriteNode { .. }
        | Node::GlobalVariableOperatorWriteNode { .. }
        | Node::ConstantWriteNode { .. }
        | Node::ConstantAndWriteNode { .. }
        | Node::ConstantOrWriteNode { .. }
        | Node::ConstantOperatorWriteNode { .. }
        | Node::ConstantPathWriteNode { .. }
        | Node::ConstantPathAndWriteNode { .. }
        | Node::ConstantPathOrWriteNode { .. }
        | Node::ConstantPathOperatorWriteNode { .. }
        | Node::CallAndWriteNode { .. }
        | Node::CallOrWriteNode { .. }
        | Node::CallOperatorWriteNode { .. }
        | Node::IndexAndWriteNode { .. }
        | Node::IndexOrWriteNode { .. }
        | Node::IndexOperatorWriteNode { .. }
        | Node::MultiWriteNode { .. } => true,
        Node::CallNode { .. } => node.as_call_node().is_some_and(|c| c.equal_loc().is_some()),
        _ => false,
    }
}

/// RuboCop's `assignment_in_and?`: whether an `and_type?` node has any
/// assignment anywhere in its subtree.
fn has_assignment_descendant(node: &Node<'_>) -> bool {
    struct Finder {
        found: bool,
    }
    impl<'pr> Visitor<'pr> for Finder {
        fn enter(&mut self, node: &Node<'pr>) {
            if is_assignment_kind(node) {
                self.found = true;
            }
        }
    }
    let mut finder = Finder { found: false };
    walk(node, &mut finder);
    finder.found
}

/// RuboCop's `Node#comparison_method?` (`COMPARISON_OPERATORS`).
fn is_comparison_method(name: &[u8]) -> bool {
    matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=" | b">" | b"<")
}

/// RuboCop's `Node#operator_method?` (`OPERATOR_METHODS`).
fn is_operator_method(name: &[u8]) -> bool {
    matches!(
        name,
        b"|" | b"^"
            | b"&"
            | b"<=>"
            | b"=="
            | b"==="
            | b"=~"
            | b">"
            | b">="
            | b"<"
            | b"<="
            | b"<<"
            | b">>"
            | b"+"
            | b"-"
            | b"*"
            | b"/"
            | b"%"
            | b"**"
            | b"~"
            | b"+@"
            | b"-@"
            | b"!@"
            | b"~@"
            | b"[]"
            | b"[]="
            | b"!"
            | b"!="
            | b"!~"
            | b"`"
    )
}

/// RuboCop's `Node#parenthesized?`: the closing delimiter is literally
/// `)`, so index calls (closing `]`) and unparenthesized command calls
/// (no closing delimiter at all) are not "parenthesized".
fn call_is_parenthesized(call: &CallNode<'_>, ctx: &Context<'_>) -> bool {
    call.closing_loc().is_some_and(|l| ctx.text(l.span()) == b")")
}

/// RuboCop's `Node#prefix_not?`: a `!`-dispatch whose selector is
/// literally the `not` keyword (as opposed to `!foo`'s `prefix_bang?`,
/// which never needs parentheses).
fn is_prefix_not(call: &CallNode<'_>, ctx: &Context<'_>) -> bool {
    call.receiver().is_some()
        && call.name().as_slice() == b"!"
        && call.message_loc().is_some_and(|l| ctx.text(l.span()) == b"not")
}

/// RuboCop's `add_parentheses?`.
fn needs_parentheses(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    if is_assignment_kind(node) {
        return true;
    }
    if matches!(node, Node::OrNode { .. }) {
        return true;
    }
    if matches!(node, Node::AndNode { .. }) && has_assignment_descendant(node) {
        return true;
    }
    let Some(call) = node.as_call_node() else { return false };
    let has_unparenthesized_args = !call.arguments().is_none_or(|a| a.arguments().is_empty())
        && !call_is_parenthesized(&call, ctx);
    has_unparenthesized_args || is_prefix_not(&call, ctx)
}

/// RuboCop's `parenthesize_method?` plus `parenthesized_method_arguments`:
/// reconstructs `method_call(args)` for an unparenthesized command call.
/// Returns `None` when the condition isn't such a call (falls through to
/// the generic `(#{source})` wrap in [`add_parentheses_if_needed`]),
/// including when it carries a block -- RuboCop's `parenthesize_method?`
/// receives the *block* node in that case, which is never `call_type?`.
fn parenthesize_method_call(condition: &Node<'_>, ctx: &Context<'_>) -> Option<Vec<u8>> {
    let call = condition.as_call_node()?;
    if call.block().is_some() {
        return None;
    }
    let args = call.arguments()?;
    let first = args.arguments().first()?;
    if call_is_parenthesized(&call, ctx) {
        return None;
    }
    let name = call.name();
    let name = name.as_slice();
    if is_comparison_method(name) || is_operator_method(name) {
        return None;
    }
    let message_span = call.message_loc()?.span();
    let node_span = condition.span();
    let method_call = ctx.text(Span::new(node_span.start, message_span.end));
    let arguments = ctx.text(Span::new(first.span().start, node_span.end));
    let mut out = Vec::with_capacity(method_call.len() + arguments.len() + 2);
    out.extend_from_slice(method_call);
    out.push(b'(');
    out.extend_from_slice(arguments);
    out.push(b')');
    Some(out)
}

/// RuboCop's `parenthesized_and`/`parenthesized_and_clause`: only the
/// rightmost clause of a chain of `&&`/`and` nodes is ever wrapped, and
/// only when it is itself an assignment; every other clause (including a
/// deeply nested assignment on the left) keeps its raw source.
fn parenthesized_and(node: &Node<'_>, ctx: &Context<'_>) -> Vec<u8> {
    let and = node.as_and_node().expect("kind matched");
    let left = ctx.text(and.left().span());
    let right = parenthesized_and_clause(&and.right(), ctx);
    let operator =
        ctx.text(ctx.with_surrounding_space(and.operator_loc().span(), Side::Both, true, true));
    let mut out = Vec::with_capacity(left.len() + operator.len() + right.len());
    out.extend_from_slice(left);
    out.extend_from_slice(operator);
    out.extend_from_slice(&right);
    out
}

fn parenthesized_and_clause(node: &Node<'_>, ctx: &Context<'_>) -> Vec<u8> {
    if matches!(node, Node::AndNode { .. }) {
        return parenthesized_and(node, ctx);
    }
    if is_assignment_kind(node) {
        let mut out = Vec::new();
        out.push(b'(');
        out.extend_from_slice(ctx.text(node.span()));
        out.push(b')');
        return out;
    }
    ctx.text(node.span()).to_vec()
}

/// RuboCop's `add_parentheses_if_needed`.
fn add_parentheses_if_needed(condition: &Node<'_>, ctx: &Context<'_>) -> Vec<u8> {
    if !needs_parentheses(condition, ctx) {
        return ctx.text(condition.span()).to_vec();
    }
    if let Some(text) = parenthesize_method_call(condition, ctx) {
        return text;
    }
    if matches!(condition, Node::AndNode { .. }) {
        return parenthesized_and(condition, ctx);
    }
    let mut out = Vec::new();
    out.push(b'(');
    out.extend_from_slice(ctx.text(condition.span()));
    out.push(b')');
    out
}

/// RuboCop's `chainable_condition`: the condition text to splice into the
/// merged condition, negated (and wrapped if needed) when the shape it
/// came from is an `unless`.
fn chainable_condition(is_if: bool, condition: &Node<'_>, ctx: &Context<'_>) -> Vec<u8> {
    let wrapped = add_parentheses_if_needed(condition, ctx);
    if is_if {
        return wrapped;
    }
    let mut out = Vec::with_capacity(wrapped.len() + 3);
    out.push(b'!');
    if matches!(condition, Node::AndNode { .. }) {
        out.push(b'(');
        out.extend_from_slice(&wrapped);
        out.push(b')');
    } else {
        out.extend_from_slice(&wrapped);
    }
    out
}

/// RuboCop's `correct_for_comment`: comments attached to the nested
/// conditional (anything sitting inside the merged-away range between the
/// outer condition and the nested one) are hoisted above the outer
/// keyword instead of being deleted along with that range.
fn hoisted_comment_text(ctx: &Context<'_>, merged_range: Span) -> Option<Vec<u8>> {
    let mut text = Vec::new();
    let mut any = false;
    for comment in ctx.comments() {
        if comment.span.start < merged_range.start || comment.span.end > merged_range.end {
            continue;
        }
        if any {
            text.push(b'\n');
        }
        text.extend_from_slice(ctx.text(comment.span));
        any = true;
    }
    if !any {
        return None;
    }
    text.push(b'\n');
    Some(text)
}

/// RuboCop's `autocorrect`, `autocorrect_outer_condition_basic` and
/// `autocorrect_outer_condition_modify_form`.
fn build_fix(outer: &Shape<'_>, inner: &Shape<'_>, ctx: &Context<'_>) -> Fix {
    let mut edits = Vec::new();
    if outer.end_span.is_none() {
        build_modify_form_fix(&mut edits, outer, inner, ctx);
    } else {
        build_basic_fix(&mut edits, outer, inner, ctx);
    }
    Fix { applicability: Applicability::Safe, edits }
}

/// RuboCop's `autocorrect_outer_condition_basic`: the outer conditional
/// keeps its own `end` (or, for the single-statement guard-style nested
/// modifier, has none to keep), so it absorbs the nested one.
fn build_basic_fix(edits: &mut Vec<Edit>, outer: &Shape<'_>, inner: &Shape<'_>, ctx: &Context<'_>) {
    if outer.is_unless {
        edits.push(Edit::replace(outer.keyword_span, b"if".to_vec()));
    }
    let outer_replacement = chainable_condition(!outer.is_unless, &outer.predicate, ctx);
    edits.push(Edit::replace(outer.predicate.span(), outer_replacement));

    if let Some(inner_end) = inner.end_span {
        // `correct_for_basic_condition_style`: the nested conditional
        // keeps its own body, dropping only its keyword/condition and its
        // now-redundant `end`.
        let merge_span = Span::new(outer.predicate.span().end, inner.predicate.span().start);
        edits.push(Edit::replace(merge_span, b" && ".to_vec()));

        let inner_replacement = chainable_condition(!inner.is_unless, &inner.predicate, ctx);
        edits.push(Edit::replace(inner.predicate.span(), inner_replacement));

        let outer_end = outer.end_span.expect("basic form has an end keyword");
        let end_range = if ctx.same_line(outer_end, inner_end) {
            outer_end
        } else {
            ctx.whole_lines(outer_end)
        };
        edits.push(Edit::delete(end_range));

        if let Some(comment_text) = hoisted_comment_text(ctx, merge_span) {
            edits.push(Edit::insert(outer.keyword_span.start, comment_text));
        }
    } else {
        // `correct_for_guard_condition_style`: the nested conditional is
        // itself a modifier (`do_something if/unless cond`).
        let inner_wrapped = chainable_condition(!inner.is_unless, &inner.predicate, ctx);
        let mut insert_text = Vec::with_capacity(inner_wrapped.len() + 4);
        insert_text.extend_from_slice(b" && ");
        insert_text.extend_from_slice(&inner_wrapped);
        edits.push(Edit::insert(outer.predicate.span().end, insert_text));

        let remove_span = ctx.with_surrounding_space(
            Span::new(inner.keyword_span.start, inner.predicate.span().end),
            Side::Both,
            false,
            false,
        );
        edits.push(Edit::delete(remove_span));
    }
}

/// RuboCop's `autocorrect_outer_condition_modify_form`: the outer
/// conditional is itself a modifier (`... if/unless cond`), so the nested
/// (non-modifier) conditional survives and absorbs the outer one instead.
fn build_modify_form_fix(
    edits: &mut Vec<Edit>,
    outer: &Shape<'_>,
    inner: &Shape<'_>,
    ctx: &Context<'_>,
) {
    if inner.is_unless {
        edits.push(Edit::replace(inner.keyword_span, b"if".to_vec()));
    }
    let outer_wrapped = chainable_condition(!outer.is_unless, &outer.predicate, ctx);
    let inner_wrapped = chainable_condition(!inner.is_unless, &inner.predicate, ctx);
    // A single merged replacement (rather than an insert butted up against
    // the following replace) sidesteps any ambiguity about ordering two
    // edits that start at the same offset.
    let mut merged = Vec::with_capacity(outer_wrapped.len() + inner_wrapped.len() + 4);
    merged.extend_from_slice(&outer_wrapped);
    merged.extend_from_slice(b" && ");
    merged.extend_from_slice(&inner_wrapped);
    edits.push(Edit::replace(inner.predicate.span(), merged));

    let remove_span = ctx.with_surrounding_space(
        Span::new(outer.keyword_span.start, outer.predicate.span().end),
        Side::Both,
        false,
        false,
    );
    edits.push(Edit::delete(remove_span));
}
