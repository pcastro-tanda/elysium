//! `Performance/BigDecimalWithNumericArgument`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/big_decimal_with_numeric_argument.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG_FROM_FLOAT_TO_STRING: &str = "Convert float literal to string and pass it to `BigDecimal`.";
const MSG_FROM_INTEGER_TO_STRING: &str =
    "Convert string literal to integer and pass it to `BigDecimal`.";

/// Identifies places where a float argument to `BigDecimal` should be converted to a string.
#[derive(Debug, Clone)]
pub struct BigDecimalWithNumericArgument {
    supported: bool,
}

impl Rule for BigDecimalWithNumericArgument {
    const META: RuleMeta = RuleMeta {
        name: "Performance/BigDecimalWithNumericArgument",
        department: Department::Performance,
        summary: "Convert numeric literal to string and pass it to `BigDecimal`.",
        explanation: "Identifies places where a float argument to `BigDecimal` should be \
                      converted to a string. Initializing from String is faster than from Float \
                      for BigDecimal.\n\nAlso identifies places where an integer string argument \
                      to BigDecimal should be converted to an integer. Initializing from Integer \
                      is faster than from String for BigDecimal.\n\n```ruby\n# bad\n\
                      BigDecimal(1.2, 3, exception: true)\n4.5.to_d(6, exception: true)\n\n\
                      # good\nBigDecimal('1.2', 3, exception: true)\n\
                      BigDecimal('4.5', 6, exception: true)\n\n# bad\nBigDecimal('1', 2)\n\
                      BigDecimal('4', 6)\n\n# good\nBigDecimal(1, 2)\n4.to_d(6)\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { supported: options.target_ruby_version() >= 3.1 })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() {
            return;
        }
        let name = call.name();
        match name.as_slice() {
            b"BigDecimal" if call.receiver().is_none() => {
                let Some(arguments) = call.arguments() else { return };
                let Some(numeric) = arguments.arguments().first() else { return };
                if is_float(&numeric) {
                    let span = numeric.span();
                    let mut text = vec![b'\''];
                    text.extend_from_slice(ctx.text(span));
                    text.push(b'\'');
                    ctx.report_with_fix(
                        &Self::META,
                        span,
                        MSG_FROM_FLOAT_TO_STRING,
                        fix(Edit::replace(span, text)),
                    );
                } else if let Some(value) = integer_string(&numeric) {
                    let span = numeric.span();
                    ctx.report_with_fix(
                        &Self::META,
                        span,
                        MSG_FROM_INTEGER_TO_STRING,
                        fix(Edit::replace(span, value)),
                    );
                }
            }
            b"to_d" => {
                let Some(numeric) = call.receiver() else { return };
                let span = numeric.span();
                let node_span = call_span_excluding_block(&call);
                if is_float(&numeric) {
                    let mut text = b"BigDecimal('".to_vec();
                    text.extend_from_slice(ctx.text(span));
                    text.push(b'\'');
                    if let Some(arguments) = call.arguments() {
                        for arg in &arguments.arguments() {
                            text.extend_from_slice(b", ");
                            text.extend_from_slice(ctx.text(arg.span()));
                        }
                    }
                    text.push(b')');
                    ctx.report_with_fix(
                        &Self::META,
                        span,
                        MSG_FROM_FLOAT_TO_STRING,
                        fix(Edit::replace(node_span, text)),
                    );
                } else if let Some(mut value) = integer_string(&numeric) {
                    value.extend_from_slice(b".to_d");
                    ctx.report_with_fix(
                        &Self::META,
                        span,
                        MSG_FROM_INTEGER_TO_STRING,
                        fix(Edit::replace(node_span, value)),
                    );
                }
            }
            _ => {}
        }
    }
}

fn fix(edit: Edit) -> Fix {
    Fix { applicability: Applicability::Safe, edits: vec![edit] }
}

fn is_float(node: &Node<'_>) -> bool {
    node.as_float_node().is_some()
}

/// A plain string literal whose value matches `/\A\d+\z/`.
fn integer_string(node: &Node<'_>) -> Option<Vec<u8>> {
    let string = node.as_string_node()?;
    let value = string.unescaped();
    (!value.is_empty() && value.iter().all(u8::is_ascii_digit)).then(|| value.to_vec())
}
