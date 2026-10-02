//! `Rails/DuplicateScope`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/duplicate_scope.rb` (with its
//! `ClassSendNodeHelper` mixin).

use std::collections::HashMap;
use std::fmt::Write as _;

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{for_each_child, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Multiple scopes share this same expression.";

/// Checks for multiple scopes in a model that have the same expression.
#[derive(Debug, Clone)]
pub struct DuplicateScope;

impl Rule for DuplicateScope {
    const META: RuleMeta = RuleMeta {
        name: "Rails/DuplicateScope",
        department: Department::Rails,
        summary: "Multiple scopes share this same expression.",
        explanation: "Checks for multiple scopes in a model that have the same expression. \
                      This often means you copy/pasted a scope, updated the name, and forgot \
                      to change the condition.\n\n```ruby\n# bad\nscope :visible, -> { \
                      where(visible: true) }\nscope :hidden, -> { where(visible: true) }\n\n\
                      # good\nscope :visible, -> { where(visible: true) }\nscope :hidden, -> { \
                      where(visible: false) }\n```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ClassNode],
        config: &[],
        blind_spots: "Scope expressions are compared by a structural fingerprint (node kinds, \
                      with each node's own punctuation and names, ignoring whitespace) rather \
                      than by RuboCop's AST equality, so e.g. `'a'` and `\"a\"` differ. A class \
                      body with `rescue` is not inspected.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(class) = node.as_class_node() else { return };
        let Some(body) = class.body() else { return };
        let Some(statements) = body.as_statements_node() else { return };

        let mut scopes: Vec<(Span, Vec<String>)> = Vec::new();
        for statement in statements.body().iter() {
            // `class_send_nodes`: only `send` children; a call with a block is a `block`.
            let Some(call) = statement.as_call_node() else { continue };
            if call.receiver().is_some()
                || call.name().as_slice() != b"scope"
                || call.block().is_some_and(|block| block.as_block_argument_node().is_none())
            {
                continue;
            }
            // `(send nil? :scope _ $...)`.
            let mut expression: Vec<String> = Vec::new();
            let mut arguments = 0;
            if let Some(list) = call.arguments() {
                for argument in list.arguments().iter() {
                    if arguments > 0 {
                        expression.push(fingerprint(&argument, ctx));
                    }
                    arguments += 1;
                }
            }
            if arguments == 0 {
                continue;
            }
            if let Some(block) = call.block() {
                expression.push(fingerprint(&block, ctx));
            }
            scopes.push((statement.span(), expression));
        }

        let mut counts: HashMap<&[String], usize> = HashMap::new();
        for (_, expression) in &scopes {
            *counts.entry(expression.as_slice()).or_default() += 1;
        }
        for (span, expression) in &scopes {
            if counts[expression.as_slice()] > 1 {
                ctx.report(&Self::META, *span, MSG);
            }
        }
    }
}

/// A whitespace-insensitive structural rendering of `node`: its kind, its
/// own source (everything outside its children), then its children's.
fn fingerprint(node: &Node<'_>, ctx: &Context<'_>) -> String {
    let mut out = String::new();
    render(node, ctx, &mut out);
    out
}

fn render(node: &Node<'_>, ctx: &Context<'_>, out: &mut String) {
    let _ = write!(out, "{:?}(", node.kind());
    let keep_space = matches!(
        node.kind(),
        NodeKind::StringNode
            | NodeKind::SymbolNode
            | NodeKind::RegularExpressionNode
            | NodeKind::XStringNode
    );
    let span = node.span();
    let mut cursor = span.start;
    let mut pieces: Vec<(Span, String)> = Vec::new();
    for_each_child(node, |child| {
        let mut rendered = String::new();
        render(child, ctx, &mut rendered);
        pieces.push((child.span(), rendered));
    });
    let gap = |from: u32, to: u32, out: &mut String| {
        if from >= to {
            return;
        }
        let text = String::from_utf8_lossy(ctx.text(Span::new(from, to)));
        if keep_space {
            out.push_str(&text);
        } else {
            out.extend(text.chars().filter(|c| !c.is_whitespace()));
        }
    };
    for (child_span, rendered) in &pieces {
        gap(cursor, child_span.start, out);
        out.push('<');
        out.push_str(rendered);
        out.push('>');
        cursor = child_span.end.max(cursor);
    }
    gap(cursor, span.end, out);
    out.push(')');
}
