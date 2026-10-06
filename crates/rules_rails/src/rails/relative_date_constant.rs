//! `Rails/RelativeDateConstant`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/relative_date_constant.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{for_each_child, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const RELATIVE_DATE_METHODS: &[&[u8]] =
    &[b"since", b"from_now", b"after", b"ago", b"until", b"before", b"yesterday", b"tomorrow"];

/// Do not assign relative date to constants.
#[derive(Debug, Clone)]
pub struct RelativeDateConstant;

impl Rule for RelativeDateConstant {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RelativeDateConstant",
        department: Department::Rails,
        summary: "Do not assign relative date to constants.",
        explanation: "Checks whether constant value isn't relative date.\nBecause the relative \
                      date will be evaluated only once.\n\nThis cop's autocorrection is unsafe \
                      because the replaced method is called on each reference, which may be \
                      slower than reading a constant.\n\n```ruby\n# bad\nclass SomeClass\n  \
                      EXPIRED_AT = 1.week.since\nend\n\n# good\nclass SomeClass\n  def self.\
                      expired_at\n    1.week.since\n  end\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantPathWriteNode,
            NodeKind::MultiWriteNode,
            NodeKind::ConstantOrWriteNode,
            NodeKind::ConstantPathOrWriteNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::ConstantWriteNode | NodeKind::ConstantPathWriteNode => {
                let Some(method) = nested_relative_date(node) else { return };
                let span = node.span();
                let fix = autocorrect(node, ctx);
                report(ctx, span, &method, fix);
            }
            NodeKind::MultiWriteNode => on_masgn(node, ctx),
            NodeKind::ConstantOrWriteNode => {
                let Some(write) = node.as_constant_or_write_node() else { return };
                or_assignment(node, &write.value(), ctx);
            }
            NodeKind::ConstantPathOrWriteNode => {
                let Some(write) = node.as_constant_path_or_write_node() else { return };
                or_assignment(node, &write.value(), ctx);
            }
            _ => {}
        }
    }
}

fn report(ctx: &mut Context<'_>, span: Span, method: &str, fix: Option<Fix>) {
    let message =
        format!("Do not assign `{method}` to constants as it will be evaluated only once.");
    match fix {
        Some(fix) => ctx.report_with_fix(&RelativeDateConstant::META, span, message, fix),
        None => ctx.report(&RelativeDateConstant::META, span, message),
    }
}

/// `(or_asgn (casgn _ _) (send _ $RELATIVE_DATE_METHODS))`.
fn or_assignment(node: &Node<'_>, value: &Node<'_>, ctx: &mut Context<'_>) {
    if let Some(method) = relative_date(value) {
        report(ctx, node.span(), &method, None);
    }
}

fn on_masgn(node: &Node<'_>, ctx: &mut Context<'_>) {
    let Some(write) = node.as_multi_write_node() else { return };
    let Some(array) = write.value().as_array_node() else { return };
    let mut targets: Vec<Node<'_>> = write.lefts().iter().collect();
    targets.extend(write.rest().filter(|r| r.as_implicit_rest_node().is_none()));
    targets.extend(write.rights().iter());
    for (name, value) in targets.iter().zip(array.elements().iter()) {
        if name.as_constant_target_node().is_none() && name.as_constant_path_target_node().is_none()
        {
            continue;
        }
        if let Some(method) = nested_relative_date(&value) {
            let span = Span::new(name.span().start, value.span().end);
            report(ctx, span, &method, None);
        }
    }
}

fn autocorrect(node: &Node<'_>, ctx: &Context<'_>) -> Option<Fix> {
    // Only an unscoped constant (`scope.nil?`).
    let write = node.as_constant_write_node()?;
    let name = String::from_utf8_lossy(write.name().as_slice()).to_lowercase();
    let value = write.value();
    let span = node.span();
    let indent = " ".repeat(ctx.line_col(span.start).column as usize);
    let value_source = String::from_utf8_lossy(ctx.text(value.span())).into_owned();
    let code = format!("def self.{name}\n{indent}{indent}{value_source}\n{indent}end");
    Some(Fix {
        applicability: Applicability::Unsafe,
        edits: vec![Edit::replace(span, code.into_bytes())],
    })
}

/// `relative_date`: `(send _ $RELATIVE_DATE_METHODS)`.
fn relative_date(node: &Node<'_>) -> Option<String> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation()
        || call.arguments().is_some()
        || call.block().is_some()
        || !RELATIVE_DATE_METHODS.contains(&call.name().as_slice())
    {
        return None;
    }
    Some(String::from_utf8_lossy(call.name().as_slice()).into_owned())
}

/// `nested_relative_date`: the first relative-date call found post-order
/// below (and including) `node`, never descending into blocks.
fn nested_relative_date(node: &Node<'_>) -> Option<String> {
    if node.as_lambda_node().is_some()
        || node
            .as_call_node()
            .is_some_and(|c| c.block().is_some_and(|b| b.as_block_node().is_some()))
    {
        return None;
    }
    let mut found = None;
    for_each_child(node, |child| {
        if found.is_none() {
            found = nested_relative_date(child);
        }
    });
    found.or_else(|| relative_date(node))
}
