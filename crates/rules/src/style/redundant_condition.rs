//! `Style/RedundantCondition`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_condition.rb` plus its `AllowedMethods`
//! and `CommentsHelp` mixins.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    NodeInfo, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, StatementsNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Use double pipes `||` instead.";
/// RuboCop's `REDUNDANT_CONDITION`.
const REDUNDANT_CONDITION: &str = "This condition is not needed.";

/// Checks for unnecessary conditional expressions.
#[derive(Debug, Clone)]
pub struct RedundantCondition {
    /// RuboCop's `AllowedMethods` (`AllowedMethods` mixin).
    allowed_methods: Vec<String>,
}

impl Rule for RedundantCondition {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantCondition",
        department: Department::Style,
        summary: "Checks for unnecessary conditional expressions.",
        explanation: "\
NOTE: Since the intention of the comment cannot be automatically determined,
autocorrection is not applied when a comment is used inside the conditional.

```ruby
# bad
a = b ? b : c

# good
a = b || c

# bad
if b
  b
else
  c
end

# good
b || c

# good
if b
  b
elsif cond
  c
end

# bad
a.nil? ? true : a

# good
a.nil? || a

# bad
if a.nil?
  true
else
  a
end

# good
a.nil? || a
```

With `AllowedMethods: ['nonzero?']` (the default), a predicate call in that
list is exempt from the \"true branch is a bare `true`\" check:

```ruby
# good
num.nonzero? ? true : false
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode],
        config: &[ConfigOption {
            name: "AllowedMethods",
            default: ConfigDefault::StrList(&["nonzero?"]),
            allowed: &[],
            doc: "Predicate methods allowed as the condition when the true branch is a bare \
                  `true` literal.",
        }],
        blind_spots: "\
Node equality (RuboCop's AST `==`, e.g. `condition == if_branch`) is
approximated by comparing the exact source text of the two spans; two
structurally-identical expressions written with different literal
formatting (extra parens, different whitespace) are treated as unequal,
matching real-world code but a narrower notion than RuboCop's true AST
comparison.

`node.parent&.send_type?` (used to decide whether the reconstructed `||`
form needs wrapping parens) is approximated from the parent's `NodeKind`
alone, so it does not distinguish a safe-navigation (`&.`) parent call from
a plain one -- both wrap, which is always safe to do.

`node.each_descendant.any? { contains_comments? }` (RuboCop's per-descendant,
per-line comment scan that blocks autocorrection) is approximated as \"any
comment anywhere inside the `if`/`unless` node's byte range\", a coarser
but strictly more conservative check: it only ever skips a fix RuboCop would
have applied, never the reverse.

A branch made of two or more `;`- or newline-separated statements that all
fit on a single source line (an implicit `begin` RuboCop can still call
`.source` on) is not reconstructed into the `||` form; the offense is
skipped entirely. Not covered by any known RuboCop spec.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allowed_methods: options.str_list("AllowedMethods") })
    }

    fn leave(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let shape = shape_of(node);
        if shape.is_modifier || !self.offense(&shape, ctx) {
            return;
        }
        let has_method_branches = branches_have_method(&shape, ctx);
        let span = range_of_offense(&shape, has_method_branches);
        let message =
            if matches!(shape.else_branch, Branch::Empty) { REDUNDANT_CONDITION } else { MSG };
        match Self::build_fix(&shape, has_method_branches, ctx) {
            Some(fix) => ctx.report_with_fix(&Self::META, span, message, fix),
            None => ctx.report(&Self::META, span, message),
        }
    }
}

impl RedundantCondition {
    /// RuboCop's `offense?`.
    fn offense(&self, shape: &Shape<'_>, ctx: &Context<'_>) -> bool {
        if use_if_branch(&shape.else_branch) || use_hash_key_assignment(&shape.else_branch) {
            return false;
        }
        self.synonymous_condition_and_branch(shape, ctx)
            && shape.keyword != Keyword::Elsif
            && (shape.keyword == Keyword::Ternary
                || !else_branch_needs_single_line_check(&shape.else_branch)
                || branch_single_line(&shape.else_branch, ctx))
    }

    /// RuboCop's `synonymous_condition_and_branch?`.
    fn synonymous_condition_and_branch(&self, shape: &Shape<'_>, ctx: &Context<'_>) -> bool {
        // e.g. `if var; var; else; 'foo'; end`
        if let Some(if_node) = branch_node(&shape.if_branch) {
            if node_text_eq(ctx, &shape.condition, if_node) {
                return true;
            }
        }
        // e.g. `a.nil? ? true : a`
        if self.if_branch_is_true_type_and_else_is_not(shape) {
            return true;
        }
        // e.g. `if foo; @value = foo; else; @value = another_value?; end`
        if let (Some(if_node), Some(else_node)) =
            (branch_node(&shape.if_branch), branch_node(&shape.else_branch))
        {
            if let (Some(if_name), Some(else_name)) = (asgn_name(if_node), asgn_name(else_node)) {
                if if_name == else_name {
                    if let Some(value) = asgn_value(if_node) {
                        if node_text_eq(ctx, &shape.condition, &value) {
                            return true;
                        }
                    }
                }
            }
        }
        // e.g. `if foo; test.value = foo; else; test.value = another_value?; end`
        if branches_have_method(shape, ctx) {
            if let Some(if_call) = branch_call(&shape.if_branch) {
                if !use_hash_key_access(&if_call) {
                    if let Some(arg) = first_argument(&if_call) {
                        if node_text_eq(ctx, &shape.condition, &arg) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// RuboCop's `if_branch_is_true_type_and_else_is_not?`.
    fn if_branch_is_true_type_and_else_is_not(&self, shape: &Shape<'_>) -> bool {
        if shape.keyword == Keyword::Unless
            || !matches!(shape.keyword, Keyword::Ternary | Keyword::If)
        {
            return false;
        }
        let Some(call) = shape.condition.as_call_node() else { return false };
        let name = call.name().as_slice();
        if !name.ends_with(b"?") || self.allowed_methods.iter().any(|m| m.as_bytes() == name) {
            return false;
        }
        if !matches!(branch_node(&shape.if_branch), Some(Node::TrueNode { .. })) {
            return false;
        }
        match branch_node(&shape.else_branch) {
            None | Some(Node::TrueNode { .. }) => false,
            Some(_) => true,
        }
    }

    /// RuboCop's `autocorrect`. Returns `None` when the offense is not
    /// autocorrectable (a comment sits inside the node), matching
    /// RuboCop's silent `return` from `autocorrect`.
    fn build_fix(shape: &Shape<'_>, has_method_branches: bool, ctx: &Context<'_>) -> Option<Fix> {
        if has_comment_within(shape.node_span, ctx) {
            return None;
        }
        if shape.keyword == Keyword::Ternary && !has_method_branches {
            return Some(correct_ternary(shape));
        }
        if matches!(shape.else_branch, Branch::Empty) {
            let span = branch_span(&shape.if_branch)?;
            let text = ctx.text(span).to_vec();
            return Some(Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(shape.node_span, text)],
            });
        }
        let form = make_ternary_form(shape, has_method_branches, ctx)?;
        Some(Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(shape.node_span, form.into_bytes())],
        })
    }
}

/// A normalized `if`/`unless` branch body, mirroring `rubocop-ast`'s
/// `IfNode#if_branch`/`#else_branch`: a single statement is unwrapped
/// (whatever its own node kind -- a `ParenthesesNode` for an explicitly
/// parenthesized branch, exactly like whitequark's `:begin` wrapping), an
/// empty body is `Empty`, and multiple statements stay as the underlying
/// `StatementsNode` (RuboCop's synthetic `begin`).
enum Branch<'pr> {
    Empty,
    Single(Node<'pr>),
    Multi(StatementsNode<'pr>),
}

fn branch_of(stmts: Option<StatementsNode<'_>>) -> Branch<'_> {
    let Some(stmts) = stmts else { return Branch::Empty };
    let body = stmts.body();
    match body.len() {
        0 => Branch::Empty,
        1 => Branch::Single(body.first().expect("len == 1")),
        _ => Branch::Multi(stmts),
    }
}

/// The branch's own source span, if it has one.
fn branch_span(branch: &Branch<'_>) -> Option<Span> {
    match branch {
        Branch::Empty => None,
        Branch::Single(n) => Some(n.span()),
        Branch::Multi(s) => Some(s.location().span()),
    }
}

/// The branch's node, when it is exactly one statement.
fn branch_node<'a, 'b>(branch: &'b Branch<'a>) -> Option<&'b Node<'a>> {
    match branch {
        Branch::Single(n) => Some(n),
        _ => None,
    }
}

/// The branch's node as a `CallNode`, when it is exactly one `send`/`csend`
/// statement.
fn branch_call<'a>(branch: &Branch<'a>) -> Option<CallNode<'a>> {
    branch_node(branch).and_then(Node::as_call_node)
}

/// A normalized view of one `if`/`unless` node, spanning both Prism node
/// kinds. `if_branch`/`else_branch` are normalized the way `IfNode#node_parts`
/// is: `if_branch` always runs when `condition`, as literally written, is
/// truthy (for `unless`, that is its `else` clause; its own body is
/// `else_branch`).
///
/// `keyword` mirrors RuboCop's unified `if` shape: whitequark's AST
/// represents `if`/`unless`/ternary as one `:if` node type, differentiated
/// only by `keyword`/`ternary?`; Prism keeps `if`/ternary as `IfNode` (with
/// `Elsif`/`If`/`Ternary` distinguished by `if_keyword_loc`) and `unless` as
/// a separate `UnlessNode`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Keyword {
    Unless,
    Ternary,
    If,
    Elsif,
}

struct Shape<'pr> {
    keyword: Keyword,
    /// RuboCop's `modifier_form?`.
    is_modifier: bool,
    condition: Node<'pr>,
    if_branch: Branch<'pr>,
    else_branch: Branch<'pr>,
    /// The ternary's `?` span, if this is a ternary.
    then_span: Option<Span>,
    /// The ternary's `:` span, if this is a ternary.
    colon_span: Option<Span>,
    node_span: Span,
}

fn shape_of<'pr>(node: &Node<'pr>) -> Shape<'pr> {
    match node {
        Node::IfNode { .. } => {
            let n = node.as_if_node().expect("kind matched");
            let kw = n.if_keyword_loc();
            let is_modifier = kw.is_some() && n.end_keyword_loc().is_none();
            let keyword = match kw.map(|l| l.as_slice()) {
                None => Keyword::Ternary,
                Some(b"elsif") => Keyword::Elsif,
                Some(_) => Keyword::If,
            };
            let if_branch = branch_of(n.statements());
            let (else_branch, colon_span) = match n.subsequent() {
                None => (Branch::Empty, None),
                Some(sub) => match &sub {
                    Node::IfNode { .. } => (Branch::Single(sub), None),
                    Node::ElseNode { .. } => {
                        let e = sub.as_else_node().expect("subsequent is an if or an else");
                        (branch_of(e.statements()), Some(e.else_keyword_loc().span()))
                    }
                    _ => unreachable!("IfNode#subsequent is an if or an else"),
                },
            };
            Shape {
                keyword,
                is_modifier,
                condition: n.predicate(),
                if_branch,
                else_branch,
                then_span: n.then_keyword_loc().map(|l| l.span()),
                colon_span,
                node_span: node.span(),
            }
        }
        Node::UnlessNode { .. } => {
            let n = node.as_unless_node().expect("kind matched");
            let if_branch = match n.else_clause() {
                None => Branch::Empty,
                Some(e) => branch_of(e.statements()),
            };
            let else_branch = branch_of(n.statements());
            Shape {
                keyword: Keyword::Unless,
                is_modifier: n.end_keyword_loc().is_none(),
                condition: n.predicate(),
                if_branch,
                else_branch,
                then_span: None,
                colon_span: None,
                node_span: node.span(),
            }
        }
        _ => unreachable!("kinds restricted to IfNode/UnlessNode"),
    }
}

/// RuboCop's `use_if_branch?`: the (normalized) else branch is itself a
/// conditional -- an `elsif` continuation, a nested `if`/`unless`, or even a
/// nested ternary (also an `:if` node in whitequark's unified AST).
fn use_if_branch(branch: &Branch<'_>) -> bool {
    matches!(branch_node(branch), Some(Node::IfNode { .. } | Node::UnlessNode { .. }))
}

/// RuboCop's `use_hash_key_assignment?`.
fn use_hash_key_assignment(branch: &Branch<'_>) -> bool {
    branch_call(branch).is_some_and(|call| call.name().as_slice() == b"[]=")
}

/// RuboCop's `use_hash_key_access?`.
fn use_hash_key_access(call: &CallNode<'_>) -> bool {
    call.name().as_slice() == b"[]"
}

/// RuboCop's `!else_branch.instance_of?(AST::Node)`, inverted: whether
/// `else_branch` needs the `single_line?` check at all. In rubocop-ast
/// almost every node type is mapped to a specific subclass (so the
/// `instance_of?(AST::Node)` check is false and this bypasses); the
/// exceptions relevant here are the literal keywords `true`/`false`/`nil`/
/// `self` and a multi-statement implicit `begin`, none of which get their
/// own subclass.
fn else_branch_needs_single_line_check(branch: &Branch<'_>) -> bool {
    match branch {
        Branch::Empty => false,
        Branch::Multi(_) => true,
        Branch::Single(n) => matches!(
            n,
            Node::TrueNode { .. }
                | Node::FalseNode { .. }
                | Node::NilNode { .. }
                | Node::SelfNode { .. }
        ),
    }
}

fn branch_single_line(branch: &Branch<'_>, ctx: &Context<'_>) -> bool {
    let Some(span) = branch_span(branch) else { return true };
    let start_line = ctx.line_col(span.start).line;
    let end_offset = if span.end > span.start { span.end - 1 } else { span.start };
    start_line == ctx.line_col(end_offset).line
}

/// RuboCop's `branches_have_assignment?`.
fn branches_have_assignment(shape: &Shape<'_>) -> bool {
    let (Some(if_node), Some(else_node)) =
        (branch_node(&shape.if_branch), branch_node(&shape.else_branch))
    else {
        return false;
    };
    matches!((asgn_name(if_node), asgn_name(else_node)), (Some(a), Some(b)) if a == b)
}

/// RuboCop's `asgn_type?`: only the plain `=` forms over local/instance/
/// class/global variables count -- not compound (`+=`/`||=`) forms and not
/// constant assignment.
fn asgn_name<'a>(node: &Node<'a>) -> Option<&'a [u8]> {
    match node {
        Node::LocalVariableWriteNode { .. } => {
            Some(node.as_local_variable_write_node().expect("kind matched").name_loc().as_slice())
        }
        Node::InstanceVariableWriteNode { .. } => Some(
            node.as_instance_variable_write_node().expect("kind matched").name_loc().as_slice(),
        ),
        Node::ClassVariableWriteNode { .. } => {
            Some(node.as_class_variable_write_node().expect("kind matched").name_loc().as_slice())
        }
        Node::GlobalVariableWriteNode { .. } => {
            Some(node.as_global_variable_write_node().expect("kind matched").name_loc().as_slice())
        }
        _ => None,
    }
}

/// The assigned value (RHS) of one of the plain assignment kinds above.
fn asgn_value<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    match node {
        Node::LocalVariableWriteNode { .. } => {
            Some(node.as_local_variable_write_node().expect("kind matched").value())
        }
        Node::InstanceVariableWriteNode { .. } => {
            Some(node.as_instance_variable_write_node().expect("kind matched").value())
        }
        Node::ClassVariableWriteNode { .. } => {
            Some(node.as_class_variable_write_node().expect("kind matched").value())
        }
        Node::GlobalVariableWriteNode { .. } => {
            Some(node.as_global_variable_write_node().expect("kind matched").value())
        }
        _ => None,
    }
}

/// RuboCop's `branches_have_method?`.
fn branches_have_method(shape: &Shape<'_>, ctx: &Context<'_>) -> bool {
    let (Some(if_call), Some(else_call)) =
        (branch_call(&shape.if_branch), branch_call(&shape.else_branch))
    else {
        return false;
    };
    single_argument_method(&if_call)
        && single_argument_method(&else_call)
        && same_method(ctx, &if_call, &else_call)
}

/// RuboCop's `single_argument_method?`.
fn single_argument_method(call: &CallNode<'_>) -> bool {
    if call.name().as_slice() == b"[]" {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let list = args.arguments();
    if list.len() != 1 {
        return false;
    }
    !argument_with_operator(&list.first().expect("len == 1"))
}

/// RuboCop's `argument_with_operator?`.
fn argument_with_operator(node: &Node<'_>) -> bool {
    match node {
        Node::SplatNode { .. }
        | Node::BlockArgumentNode { .. }
        | Node::ForwardingArgumentsNode { .. } => true,
        Node::KeywordHashNode { .. } => {
            let kw = node.as_keyword_hash_node().expect("kind matched");
            matches!(kw.elements().first(), Some(Node::AssocSplatNode { .. }))
        }
        _ => false,
    }
}

/// RuboCop's `same_method?`.
fn same_method(ctx: &Context<'_>, a: &CallNode<'_>, b: &CallNode<'_>) -> bool {
    a.name().as_slice() == b.name().as_slice() && receiver_eq(ctx, a.receiver(), b.receiver())
}

fn receiver_eq(ctx: &Context<'_>, a: Option<Node<'_>>, b: Option<Node<'_>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => node_text_eq(ctx, &x, &y),
        _ => false,
    }
}

fn node_text_eq(ctx: &Context<'_>, a: &Node<'_>, b: &Node<'_>) -> bool {
    ctx.text(a.span()) == ctx.text(b.span())
}

fn first_argument<'a>(call: &CallNode<'a>) -> Option<Node<'a>> {
    call.arguments()?.arguments().first()
}

fn is_parenthesized_call(call: &CallNode<'_>) -> bool {
    call.opening_loc().is_some_and(|o| o.as_slice() == b"(")
}

/// RuboCop's `use_arithmetic_operation?`.
fn is_arithmetic_operation(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|call| {
        matches!(call.name().as_slice(), b"+" | b"-" | b"*" | b"/" | b"%" | b"**")
    })
}

/// RuboCop's `require_parentheses?`.
fn requires_parentheses(node: &Node<'_>) -> bool {
    match node {
        Node::IfNode { .. } => {
            let n = node.as_if_node().expect("kind matched");
            n.if_keyword_loc().is_some() && n.end_keyword_loc().is_none()
        }
        Node::UnlessNode { .. } => {
            node.as_unless_node().expect("kind matched").end_keyword_loc().is_none()
        }
        Node::WhileNode { .. } => {
            node.as_while_node().expect("kind matched").closing_loc().is_none()
        }
        Node::UntilNode { .. } => {
            node.as_until_node().expect("kind matched").closing_loc().is_none()
        }
        Node::RangeNode { .. } | Node::RescueModifierNode { .. } => true,
        Node::AndNode { .. } => {
            node.as_and_node().expect("kind matched").operator_loc().as_slice() == b"and"
        }
        Node::OrNode { .. } => {
            node.as_or_node().expect("kind matched").operator_loc().as_slice() == b"or"
        }
        _ => false,
    }
}

/// RuboCop's `require_braces?`: a hash-like literal without explicit `{`/`}`
/// (rubocop-ast's `hash_type? && !braces?`; Prism gives such a hash its own
/// `KeywordHashNode` kind rather than a braces flag on `HashNode`).
fn requires_braces(node: &Node<'_>) -> bool {
    matches!(node, Node::KeywordHashNode { .. })
}

/// RuboCop's `OPERATOR_METHODS`.
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

/// RuboCop's `assignment_method?`.
fn is_assignment_method(name: &[u8]) -> bool {
    name.ends_with(b"=") && !matches!(name, b"==" | b"===" | b"<=" | b">=" | b"!=")
}

/// RuboCop's `without_argument_parentheses_method?`.
fn is_without_argument_parens_method(call: &CallNode<'_>) -> bool {
    let has_args = call.arguments().is_some_and(|a| !a.arguments().is_empty());
    has_args
        && !is_parenthesized_call(call)
        && !is_operator_method(call.name().as_slice())
        && !is_assignment_method(call.name().as_slice())
}

fn without_argument_parens_source(call: &CallNode<'_>, ctx: &Context<'_>) -> String {
    let name = String::from_utf8_lossy(call.name().as_slice()).into_owned();
    let args: Vec<String> = call
        .arguments()
        .map(|a| a.arguments().iter().map(|arg| text(ctx, arg.span())).collect())
        .unwrap_or_default();
    format!("{name}({})", args.join(", "))
}

fn text(ctx: &Context<'_>, span: Span) -> String {
    String::from_utf8_lossy(ctx.text(span)).into_owned()
}

fn has_comment_within(span: Span, ctx: &Context<'_>) -> bool {
    ctx.comments().iter().any(|c| c.span.start >= span.start && c.span.end <= span.end)
}

/// RuboCop's `range_of_offense`.
fn range_of_offense(shape: &Shape<'_>, has_method_branches: bool) -> Span {
    if shape.keyword != Keyword::Ternary || has_method_branches {
        return shape.node_span;
    }
    match (shape.then_span, shape.colon_span) {
        (Some(then), Some(colon)) => Span::new(then.start, colon.end),
        _ => shape.node_span,
    }
}

/// RuboCop's `correct_ternary`.
fn correct_ternary(shape: &Shape<'_>) -> Fix {
    let then = shape.then_span.expect("ternary has a '?'");
    let colon = shape.colon_span.expect("ternary has a ':'");
    let mut edits = vec![Edit::replace(Span::new(then.start, colon.end), b"||".to_vec())];
    if matches!(branch_node(&shape.else_branch), Some(Node::RangeNode { .. })) {
        if let Some(span) = branch_span(&shape.else_branch) {
            edits.push(Edit::insert(span.start, b"(".to_vec()));
            edits.push(Edit::insert(span.end, b")".to_vec()));
        }
    }
    Fix { applicability: Applicability::Safe, edits }
}

/// RuboCop's `make_ternary_form`. `None` for a shape this engine cannot
/// reconstruct a `||` form for (a multi-statement branch), matching this
/// rule's documented blind spot; the offense is then dropped rather than
/// risking an incorrect fix.
fn make_ternary_form(
    shape: &Shape<'_>,
    has_method_branches: bool,
    ctx: &Context<'_>,
) -> Option<String> {
    let arithmetic = branch_node(&shape.if_branch).is_some_and(is_arithmetic_operation);
    let has_assignment_branches = branches_have_assignment(shape);

    let if_src = if_source(shape, arithmetic, has_method_branches, ctx)?;
    let else_src = else_source(
        &shape.else_branch,
        arithmetic,
        has_method_branches,
        has_assignment_branches,
        ctx,
    )?;

    let mut form = format!("{if_src} || {else_src}");
    if has_method_branches {
        if let Some(call) = branch_call(&shape.if_branch) {
            if is_parenthesized_call(&call) {
                form.push(')');
            }
        }
    }
    if is_argument_of_call(ctx) {
        form = format!("({form})");
    }
    Some(form)
}

/// RuboCop's `node.parent&.send_type?`. In whitequark's AST a `send`
/// node's arguments are direct children of the send node itself, so a
/// keyword-`if` used as a call argument has that call as its immediate
/// parent; Prism instead wraps call arguments in an `ArgumentsNode`, so the
/// immediate parent here is one level too deep -- check up to the
/// grandparent too.
fn is_argument_of_call(ctx: &Context<'_>) -> bool {
    let ancestors = ctx.ancestors();
    matches!(ancestors.last(), Some(NodeInfo { kind: NodeKind::CallNode, .. }))
        || (matches!(ancestors.last(), Some(NodeInfo { kind: NodeKind::ArgumentsNode, .. }))
            && matches!(
                ancestors.get(ancestors.len().wrapping_sub(2)),
                Some(NodeInfo { kind: NodeKind::CallNode, .. })
            ))
}

/// RuboCop's `if_source`.
fn if_source(
    shape: &Shape<'_>,
    arithmetic: bool,
    has_method_branches: bool,
    ctx: &Context<'_>,
) -> Option<String> {
    if let Some(call) = branch_call(&shape.if_branch) {
        if has_method_branches && is_parenthesized_call(&call) {
            let full = text(ctx, call.location().span());
            return Some(full[..full.len() - 1].to_string());
        }
        if arithmetic {
            if let Some(arg) = first_argument(&call) {
                let receiver = call.receiver().map(|r| text(ctx, r.span())).unwrap_or_default();
                let method = String::from_utf8_lossy(call.name().as_slice()).into_owned();
                return Some(format!("{receiver} {method} ({}", text(ctx, arg.span())));
            }
        }
    }
    if matches!(branch_node(&shape.if_branch), Some(Node::TrueNode { .. })) {
        return Some(true_type_if_source(&shape.condition, ctx));
    }
    match &shape.if_branch {
        Branch::Multi(_) => None,
        _ => branch_span(&shape.if_branch).map(|span| text(ctx, span)),
    }
}

/// RuboCop's `if_source`'s `if_branch.true_type?` case.
fn true_type_if_source(condition: &Node<'_>, ctx: &Context<'_>) -> String {
    let Some(call) = condition.as_call_node() else { return text(ctx, condition.span()) };
    let args_empty = call.arguments().is_none_or(|a| a.arguments().is_empty());
    if args_empty || is_parenthesized_call(&call) {
        return text(ctx, condition.span());
    }
    wrap_arguments_with_parens(&call, ctx)
}

/// RuboCop's `wrap_arguments_with_parens`.
fn wrap_arguments_with_parens(call: &CallNode<'_>, ctx: &Context<'_>) -> String {
    let call_span = call.location().span();
    let message_end = call.message_loc().map_or(call_span.start, |l| l.span().end);
    let method_text = text(ctx, Span::new(call_span.start, message_end));
    let Some(first_arg) = first_argument(call) else { return text(ctx, call_span) };
    let args_text = text(ctx, Span::new(first_arg.span().start, call_span.end));
    format!("{method_text}({args_text})")
}

/// RuboCop's `else_source`.
fn else_source(
    else_branch: &Branch<'_>,
    arithmetic: bool,
    has_method_branches: bool,
    has_assignment_branches: bool,
    ctx: &Context<'_>,
) -> Option<String> {
    if arithmetic {
        if let Some(arg) = branch_call(else_branch).and_then(|call| first_argument(&call)) {
            return Some(format!("{})", text(ctx, arg.span())));
        }
    }
    if has_method_branches {
        if let Some(arg) = branch_call(else_branch).and_then(|call| first_argument(&call)) {
            if requires_parentheses(&arg) {
                return Some(format!("({})", text(ctx, arg.span())));
            }
            if requires_braces(&arg) {
                return Some(format!("{{ {} }}", text(ctx, arg.span())));
            }
            return Some(text(ctx, arg.span()));
        }
    }
    if let Some(n) = branch_node(else_branch) {
        if requires_parentheses(n) {
            return Some(format!("({})", text(ctx, n.span())));
        }
    }
    if let Some(call) = branch_call(else_branch) {
        if is_without_argument_parens_method(&call) {
            return Some(without_argument_parens_source(&call, ctx));
        }
    }
    if has_assignment_branches {
        if let Some(value) = branch_node(else_branch).and_then(asgn_value) {
            if requires_parentheses(&value) {
                return Some(format!("({})", text(ctx, value.span())));
            }
            if requires_braces(&value) {
                return Some(format!("{{ {} }}", text(ctx, value.span())));
            }
            return Some(text(ctx, value.span()));
        }
    }
    match else_branch {
        Branch::Multi(_) => None,
        _ => branch_span(else_branch).map(|span| text(ctx, span)),
    }
}
