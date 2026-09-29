//! `Style/WhileUntilModifier`, ported from RuboCop's
//! `lib/rubocop/cop/style/while_until_modifier.rb` (plus the `StatementModifier`
//! mixin it includes -- copied privately here since it is shared with
//! `Style/IfUnlessModifier`, which this port does not touch).

use std::collections::HashSet;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{Location, StatementsNode};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Favor modifier `{keyword}` usage when having a single-line body.";

/// Favor modifier `while`/`until` usage when you have a single-line body.
#[derive(Debug, Clone)]
pub struct WhileUntilModifier {
    /// `Layout/LineLength`'s `Max`, or `None` when that cop is disabled.
    max_line_length: Option<i64>,
    /// `Layout/IndentationStyle`'s `IndentationWidth`, else
    /// `Layout/IndentationWidth`'s `Width`, else `2` (RuboCop's
    /// `tab_indentation_width`); used to weigh a line's leading tabs as this
    /// many columns each when checking line length.
    tab_indentation_width: i64,
    /// Spans whose parent shape means a `while`/`until` there needs
    /// parentheses when written in modifier form (RuboCop's `parenthesize?`),
    /// built from a single whole-file traversal instead of parent pointers.
    paren_targets: HashSet<Span>,
}

impl WhileUntilModifier {
    /// Reads another cop's boolean option, falling back to `default` when
    /// unconfigured (peer reads have no schema-driven default).
    fn peer_bool(options: &RuleOptions, cop: &str, key: &str, default: bool) -> bool {
        options.peer(cop, key).and_then(OptionValue::as_bool).unwrap_or(default)
    }

    /// Reads another cop's integer option, falling back to `default`.
    fn peer_int(options: &RuleOptions, cop: &str, key: &str, default: i64) -> i64 {
        options.peer(cop, key).and_then(OptionValue::as_int).unwrap_or(default)
    }

    /// RuboCop's `line_length`: character count plus the extra columns
    /// contributed by leading tabs.
    fn line_length(&self, line: &[u8]) -> i64 {
        i64::from(ruby_source::char_len(line)) + self.indentation_difference(line)
    }

    /// RuboCop's `indentation_difference`: leading-tab count times
    /// `tab_indentation_width - 1` (a tab counts as one byte/char but
    /// `tab_indentation_width` display columns).
    fn indentation_difference(&self, line: &[u8]) -> i64 {
        let leading_tabs = line.iter().take_while(|&&b| b == b'\t').count();
        i64::try_from(leading_tabs).unwrap_or(i64::MAX) * (self.tab_indentation_width - 1)
    }
}

impl Rule for WhileUntilModifier {
    const META: RuleMeta = RuleMeta {
        name: "Style/WhileUntilModifier",
        department: Department::Style,
        summary: "Favor modifier while/until usage when you have a single-line body.",
        explanation: "\
Checks for `while` and `until` statements that would fit on one line if
written as a modifier `while`/`until`. The maximum line length is configured
in the `Layout/LineLength` cop.

```ruby
# bad
while x < 10
  x += 1
end

# good
x += 1 while x < 10

# good
while x < 10
  y += 1 if x.odd?
end

# bad
until x > 10
  x += 1
end

# good
x += 1 until x > 10

# good
until x > 10
  y += 1 unless x.even?
end

# bad
x += 100 while x < 500 # a long comment that makes code too long if it were a single line

# good
while x < 500 # a long comment that makes code too long if it were a single line
  x += 100
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::WhileNode,
            NodeKind::UntilNode,
            NodeKind::AndNode,
            NodeKind::OrNode,
            NodeKind::ArrayNode,
            NodeKind::AssocNode,
            NodeKind::CallNode,
            NodeKind::LocalVariableWriteNode,
            NodeKind::LocalVariableOperatorWriteNode,
            NodeKind::LocalVariableAndWriteNode,
            NodeKind::LocalVariableOrWriteNode,
            NodeKind::InstanceVariableWriteNode,
            NodeKind::InstanceVariableOperatorWriteNode,
            NodeKind::InstanceVariableAndWriteNode,
            NodeKind::InstanceVariableOrWriteNode,
            NodeKind::ClassVariableWriteNode,
            NodeKind::ClassVariableOperatorWriteNode,
            NodeKind::ClassVariableAndWriteNode,
            NodeKind::ClassVariableOrWriteNode,
            NodeKind::GlobalVariableWriteNode,
            NodeKind::GlobalVariableOperatorWriteNode,
            NodeKind::GlobalVariableAndWriteNode,
            NodeKind::GlobalVariableOrWriteNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantOperatorWriteNode,
            NodeKind::ConstantAndWriteNode,
            NodeKind::ConstantOrWriteNode,
            NodeKind::ConstantPathWriteNode,
            NodeKind::ConstantPathOperatorWriteNode,
            NodeKind::ConstantPathAndWriteNode,
            NodeKind::ConstantPathOrWriteNode,
            NodeKind::IndexOperatorWriteNode,
            NodeKind::IndexAndWriteNode,
            NodeKind::IndexOrWriteNode,
            NodeKind::CallOperatorWriteNode,
            NodeKind::CallAndWriteNode,
            NodeKind::CallOrWriteNode,
            NodeKind::MultiWriteNode,
        ],
        config: &[],
        blind_spots: "\
`Node#parent` (used by `parenthesize?`) is reconstructed from a whole-file
traversal recording every node shape known to need parentheses (assignment
targets, `&&`/`||`, array elements, hash values, call receivers/arguments)
rather than true parent pointers; a `while`/`until` in some other position
(e.g. a block argument) is treated as never needing parentheses, which only
risks false negatives.

`if_body_source`'s omitted-hash-value reconstruction only special-cases a
call whose last argument is a hash/keyword-hash with a value-omitted last
pair (`obj.foo bar:`); other RuboCop-recognized shapes for that rewrite fall
back to the body's raw source, which is usually byte-identical anyway.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let line_length_enabled = Self::peer_bool(options, "Layout/LineLength", "Enabled", true);
        let max_line_length = if line_length_enabled {
            Some(Self::peer_int(options, "Layout/LineLength", "Max", 120))
        } else {
            None
        };
        let tab_indentation_width = options
            .peer("Layout/IndentationStyle", "IndentationWidth")
            .and_then(OptionValue::as_int)
            .unwrap_or_else(|| Self::peer_int(options, "Layout/IndentationWidth", "Width", 2));
        Ok(Self { max_line_length, tab_indentation_width, paren_targets: HashSet::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.paren_targets.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::AndNode { .. } => {
                let n = node.as_and_node().expect("kind matched");
                self.paren_targets.insert(n.left().span());
                self.paren_targets.insert(n.right().span());
            }
            Node::OrNode { .. } => {
                let n = node.as_or_node().expect("kind matched");
                self.paren_targets.insert(n.left().span());
                self.paren_targets.insert(n.right().span());
            }
            Node::ArrayNode { .. } => {
                let n = node.as_array_node().expect("kind matched");
                for element in &n.elements() {
                    self.paren_targets.insert(element.span());
                }
            }
            Node::AssocNode { .. } => {
                let n = node.as_assoc_node().expect("kind matched");
                self.paren_targets.insert(n.value().span());
            }
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("kind matched");
                if let Some(receiver) = call.receiver() {
                    self.paren_targets.insert(receiver.span());
                }
                if let Some(arguments) = call.arguments() {
                    for argument in &arguments.arguments() {
                        self.paren_targets.insert(argument.span());
                    }
                }
            }
            Node::WhileNode { .. } => {
                let w = node.as_while_node().expect("kind matched");
                self.check_loop(
                    node.span(),
                    &w.keyword_loc(),
                    w.closing_loc().as_ref(),
                    w.predicate(),
                    w.statements(),
                    "while",
                    ctx,
                );
            }
            Node::UntilNode { .. } => {
                let u = node.as_until_node().expect("kind matched");
                self.check_loop(
                    node.span(),
                    &u.keyword_loc(),
                    u.closing_loc().as_ref(),
                    u.predicate(),
                    u.statements(),
                    "until",
                    ctx,
                );
            }
            _ => {
                if let Some(value) = assignment_value(node) {
                    self.paren_targets.insert(value.span());
                }
            }
        }
    }
}

impl WhileUntilModifier {
    /// RuboCop's `on_while`/`on_until` plus `single_line_as_modifier?`
    /// (`non_eligible_node?`, `non_eligible_body?` -- overridden to also
    /// reject a conditional body -- and `non_eligible_condition?`) and
    /// `modifier_fits_on_single_line?`.
    #[allow(clippy::too_many_arguments)]
    fn check_loop<'pr>(
        &self,
        node_span: Span,
        keyword_loc: &Location<'pr>,
        closing_loc: Option<&Location<'pr>>,
        condition: Node<'pr>,
        statements: Option<StatementsNode<'pr>>,
        keyword: &'static str,
        ctx: &mut Context<'_>,
    ) {
        let keyword_span = keyword_loc.span();

        // `non_eligible_node?`: `node.modifier_form?` (no `end` keyword,
        // including the `begin...end while`/`until` post-condition form,
        // whose own `closing_loc` is likewise absent).
        let Some(closing) = closing_loc.as_ref() else { return };
        // `node.nonempty_line_count > 3`.
        if nonempty_line_count(ctx, node_span) > 3 {
            return;
        }
        // `processed_source.line_with_comment?(node.loc.last_line)`.
        let last_line = ctx.line_col(closing.span().start).line;
        if ctx.comments().iter().any(|c| c.line == last_line) {
            return;
        }
        // `first_line_comment(node) && code_after(node)`.
        let first_line = ctx.line_col(node_span.start).line;
        let has_first_line_comment = first_line_comment(ctx, first_line).is_some();
        if has_first_line_comment && code_after(ctx, closing).is_some() {
            return;
        }

        // `non_eligible_body?`: `body&.conditional? || body.nil? ||
        // body.empty_source? || body.begin_type? ||
        // processed_source.contains_comment?(body.source_range)`.
        let Some(stmts) = statements else { return };
        let items: Vec<Node<'_>> = stmts.body().iter().collect();
        if items.is_empty() || items.len() > 1 {
            return;
        }
        let stmt = &items[0];
        if is_conditional_kind(stmt.kind()) {
            return;
        }
        let body_span = stmts.location().span();
        let body_first_line = ctx.line_col(body_span.start).line;
        let body_last_line = ctx.line_col(body_span.end.saturating_sub(1)).line;
        if ctx.comments().iter().any(|c| c.line >= body_first_line && c.line <= body_last_line) {
            return;
        }

        // `non_eligible_condition?`: `condition.each_node.any?(&:lvasgn_type?)`.
        if condition_has_lvasgn(&condition) {
            return;
        }

        // `modifier_fits_on_single_line?`.
        if !self.modifier_fits(ctx, node_span, keyword_span, keyword, &condition, stmt, closing) {
            return;
        }

        let message = MSG.replace("{keyword}", keyword);
        let fix = self.to_modifier_form_fix(ctx, node_span, keyword, &condition, stmt);
        ctx.report_with_fix(&Self::META, keyword_span, message, fix);
    }

    /// RuboCop's `modifier_fits_on_single_line?` / `line_in_modifier_form`:
    /// the rendered modifier form is compared against the bare maximum, not
    /// `acceptable_line_length?` -- `Layout/LineLength`'s exemptions (like
    /// `AllowURI`) describe long lines the user tolerates, not permission to
    /// manufacture new ones.
    #[allow(clippy::too_many_arguments)]
    fn modifier_fits(
        &self,
        ctx: &Context<'_>,
        node_span: Span,
        keyword_span: Span,
        keyword: &'static str,
        condition: &Node<'_>,
        body_stmt: &Node<'_>,
        closing_loc: &Location<'_>,
    ) -> bool {
        let Some(max) = self.max_line_length else { return true };
        let keyword_line = ctx.line_col(keyword_span.start).line;
        let keyword_col = ctx.line_col(keyword_span.start).column as usize;
        let line_text = ctx.line_text(keyword_line);
        let code_before = &line_text[..keyword_col.min(line_text.len())];
        let expression = self.modifier_expression(ctx, node_span, keyword, condition, body_stmt);
        let after = code_after(ctx, closing_loc).unwrap_or(&[]);
        let mut full = code_before.to_vec();
        full.extend_from_slice(&expression);
        full.extend_from_slice(after);
        self.line_length(&full) <= max
    }

    /// RuboCop's `to_modifier_form`.
    fn modifier_expression(
        &self,
        ctx: &Context<'_>,
        node_span: Span,
        keyword: &'static str,
        condition: &Node<'_>,
        body_stmt: &Node<'_>,
    ) -> Vec<u8> {
        let body_source = if_body_source(ctx, body_stmt);
        let mut expr = body_source;
        expr.push(b' ');
        expr.extend_from_slice(keyword.as_bytes());
        expr.push(b' ');
        expr.extend_from_slice(ctx.text(condition.span()));
        let mut out = if self.paren_targets.contains(&node_span) {
            let mut p = Vec::with_capacity(expr.len() + 2);
            p.push(b'(');
            p.extend_from_slice(&expr);
            p.push(b')');
            p
        } else {
            expr
        };
        if let Some(comment) = first_line_comment(ctx, ctx.line_col(node_span.start).line) {
            out.push(b' ');
            out.extend_from_slice(comment);
        }
        out
    }

    fn to_modifier_form_fix(
        &self,
        ctx: &Context<'_>,
        node_span: Span,
        keyword: &'static str,
        condition: &Node<'_>,
        body_stmt: &Node<'_>,
    ) -> Fix {
        let replacement = self.modifier_expression(ctx, node_span, keyword, condition, body_stmt);
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(node_span, replacement)],
        }
    }
}

/// RuboCop's `conditional?`: `if`/`while`/`until`/`case`/`case/in`.
fn is_conditional_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::IfNode
            | NodeKind::UnlessNode
            | NodeKind::WhileNode
            | NodeKind::UntilNode
            | NodeKind::CaseNode
            | NodeKind::CaseMatchNode
    )
}

/// True when `node` is whitequark's bare `lvasgn` type: a plain local
/// variable write, one of its compound-assignment variants (which
/// desugar to an outer `op_asgn`/`and_asgn`/`or_asgn` wrapping an inner
/// bare `lvasgn` target in whitequark), or a multiple-assignment target.
fn is_lvasgn_kind(node: &Node<'_>) -> bool {
    matches!(
        node,
        Node::LocalVariableWriteNode { .. }
            | Node::LocalVariableOperatorWriteNode { .. }
            | Node::LocalVariableAndWriteNode { .. }
            | Node::LocalVariableOrWriteNode { .. }
            | Node::LocalVariableTargetNode { .. }
    )
}

/// RuboCop's `condition.each_node.any?(&:lvasgn_type?)`: `condition` itself
/// or any descendant is a local variable write.
fn condition_has_lvasgn(condition: &Node<'_>) -> bool {
    if is_lvasgn_kind(condition) {
        return true;
    }
    let mut found = false;
    each_descendant(condition, &mut |child| {
        if !found && is_lvasgn_kind(child) {
            found = true;
        }
    });
    found
}

/// RuboCop's `nonempty_line_count`: non-blank lines within `span`.
fn nonempty_line_count(ctx: &Context<'_>, span: Span) -> u32 {
    let first = ctx.line_col(span.start).line;
    let last = ctx.line_col(span.end.saturating_sub(1)).line;
    u32::try_from(
        (first..=last)
            .filter(|&line| !ctx.line_text(line).iter().all(u8::is_ascii_whitespace))
            .count(),
    )
    .unwrap_or(u32::MAX)
}

/// RuboCop's `first_line_comment`: a comment on `line`, unless it disables
/// this cop (or `all`) via `# rubocop:disable`/`todo`.
fn first_line_comment<'a>(ctx: &Context<'a>, line: u32) -> Option<&'a [u8]> {
    let comment = ctx.comments().iter().find(|c| c.line == line)?;
    let disables_us = ctx.directives().directives().iter().any(|d| {
        d.span.start >= comment.span.start
            && d.span.end <= comment.span.end
            && d.kind.disables()
            && d.cops.iter().any(|c| c.covers("Style/WhileUntilModifier"))
    });
    if disables_us {
        None
    } else {
        Some(ctx.text(comment.span))
    }
}

/// RuboCop's `code_after`: non-empty trailing text on `end_loc`'s line.
fn code_after<'a>(ctx: &Context<'a>, end_loc: &Location<'_>) -> Option<&'a [u8]> {
    let end_span = end_loc.span();
    let line = ctx.line_col(end_span.end.saturating_sub(1)).line;
    let line_span = ctx.line_span(line);
    let rel = usize::try_from(end_span.end.saturating_sub(line_span.start)).unwrap_or(0);
    let text = ctx.line_text(line);
    if rel >= text.len() {
        None
    } else {
        let code = &text[rel..];
        if code.is_empty() {
            None
        } else {
            Some(code)
        }
    }
}

/// RuboCop's `if_body_source`: usually the statement's raw source, except
/// for a parenthesis-less call whose last argument is a hash/keyword-hash
/// ending in an omitted value (`obj.foo bar:`), which gets parenthesized.
fn if_body_source(ctx: &Context<'_>, stmt: &Node<'_>) -> Vec<u8> {
    if let Node::CallNode { .. } = stmt {
        let call = stmt.as_call_node().expect("kind matched");
        if call.name().as_slice() != b"[]=" {
            if let Some(arguments) = call.arguments() {
                let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
                if let Some(last) = args.last() {
                    if omitted_value_in_last_hash_arg(last) {
                        let mut out = method_source(ctx, &call);
                        out.push(b'(');
                        for (i, arg) in args.iter().enumerate() {
                            if i > 0 {
                                out.extend_from_slice(b", ");
                            }
                            out.extend_from_slice(ctx.text(arg.span()));
                        }
                        out.push(b')');
                        return out;
                    }
                }
            }
        }
    }
    ctx.text(stmt.span()).to_vec()
}

/// True when `node` is a hash/keyword-hash whose last pair's value was
/// omitted (`{ foo: }` / `foo:`).
fn omitted_value_in_last_hash_arg(node: &Node<'_>) -> bool {
    let elements = match node {
        Node::KeywordHashNode { .. } => {
            node.as_keyword_hash_node().expect("kind matched").elements()
        }
        Node::HashNode { .. } => node.as_hash_node().expect("kind matched").elements(),
        _ => return false,
    };
    let Some(last) = elements.last() else { return false };
    let Node::AssocNode { .. } = &last else { return false };
    let assoc = last.as_assoc_node().expect("kind matched");
    matches!(assoc.value(), Node::ImplicitNode { .. })
}

/// RuboCop's `method_source`: the call's own source from its start through
/// its method name (or, for an implicit `.()`/`&.()` call, through the call
/// operator).
fn method_source(ctx: &Context<'_>, call: &ruby_ast::node::CallNode<'_>) -> Vec<u8> {
    let start = call.location().span().start;
    let end = match call.message_loc() {
        Some(loc) => loc.span().end,
        None => call.call_operator_loc().map_or(call.location().span().end, |loc| loc.span().end),
    };
    ctx.text(Span::new(start, end)).to_vec()
}

/// The assigned value of every node rubocop-ast's `assignment?` matches
/// (`equals_asgn?` -- `lvasgn`/`ivasgn`/`cvasgn`/`gvasgn`/`casgn`/`masgn` --
/// or `shorthand_asgn?` -- `op_asgn`/`or_asgn`/`and_asgn`), in Prism terms.
fn assignment_value<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    Some(match node {
        Node::LocalVariableWriteNode { .. } => node.as_local_variable_write_node()?.value(),
        Node::LocalVariableOperatorWriteNode { .. } => {
            node.as_local_variable_operator_write_node()?.value()
        }
        Node::LocalVariableAndWriteNode { .. } => node.as_local_variable_and_write_node()?.value(),
        Node::LocalVariableOrWriteNode { .. } => node.as_local_variable_or_write_node()?.value(),
        Node::InstanceVariableWriteNode { .. } => node.as_instance_variable_write_node()?.value(),
        Node::InstanceVariableOperatorWriteNode { .. } => {
            node.as_instance_variable_operator_write_node()?.value()
        }
        Node::InstanceVariableAndWriteNode { .. } => {
            node.as_instance_variable_and_write_node()?.value()
        }
        Node::InstanceVariableOrWriteNode { .. } => {
            node.as_instance_variable_or_write_node()?.value()
        }
        Node::ClassVariableWriteNode { .. } => node.as_class_variable_write_node()?.value(),
        Node::ClassVariableOperatorWriteNode { .. } => {
            node.as_class_variable_operator_write_node()?.value()
        }
        Node::ClassVariableAndWriteNode { .. } => node.as_class_variable_and_write_node()?.value(),
        Node::ClassVariableOrWriteNode { .. } => node.as_class_variable_or_write_node()?.value(),
        Node::GlobalVariableWriteNode { .. } => node.as_global_variable_write_node()?.value(),
        Node::GlobalVariableOperatorWriteNode { .. } => {
            node.as_global_variable_operator_write_node()?.value()
        }
        Node::GlobalVariableAndWriteNode { .. } => {
            node.as_global_variable_and_write_node()?.value()
        }
        Node::GlobalVariableOrWriteNode { .. } => node.as_global_variable_or_write_node()?.value(),
        Node::ConstantWriteNode { .. } => node.as_constant_write_node()?.value(),
        Node::ConstantOperatorWriteNode { .. } => node.as_constant_operator_write_node()?.value(),
        Node::ConstantAndWriteNode { .. } => node.as_constant_and_write_node()?.value(),
        Node::ConstantOrWriteNode { .. } => node.as_constant_or_write_node()?.value(),
        Node::ConstantPathWriteNode { .. } => node.as_constant_path_write_node()?.value(),
        Node::ConstantPathOperatorWriteNode { .. } => {
            node.as_constant_path_operator_write_node()?.value()
        }
        Node::ConstantPathAndWriteNode { .. } => node.as_constant_path_and_write_node()?.value(),
        Node::ConstantPathOrWriteNode { .. } => node.as_constant_path_or_write_node()?.value(),
        Node::IndexOperatorWriteNode { .. } => node.as_index_operator_write_node()?.value(),
        Node::IndexAndWriteNode { .. } => node.as_index_and_write_node()?.value(),
        Node::IndexOrWriteNode { .. } => node.as_index_or_write_node()?.value(),
        Node::CallOperatorWriteNode { .. } => node.as_call_operator_write_node()?.value(),
        Node::CallAndWriteNode { .. } => node.as_call_and_write_node()?.value(),
        Node::CallOrWriteNode { .. } => node.as_call_or_write_node()?.value(),
        Node::MultiWriteNode { .. } => node.as_multi_write_node()?.value(),
        _ => return None,
    })
}
