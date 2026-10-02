//! `Security/IoMethods`, ported from RuboCop's
//! `lib/rubocop/cop/security/io_methods.rb`.
//!
//! Upstream's own `@safety` note is about the *check*, not the fix: a
//! string argument that isn't a pipe-prefixed command is assumed to be a
//! file path, but it could actually be something else the caller feeds to
//! a subprocess intentionally, hence a possible false positive. The
//! correction itself, once the cop does fire, is a plain deterministic text
//! replacement (`corrector.replace(receiver, 'File')`, no dynamic content),
//! so it maps to [`FixAvailability::Safe`] / [`Applicability::Safe`] here,
//! the same way other `Safe: false` cops with a literal-replace corrector
//! do in this codebase.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// Checks for the first argument to `IO.read`, `IO.binread`, `IO.write`,
/// `IO.binwrite`, `IO.foreach`, and `IO.readlines`.
#[derive(Debug, Clone)]
pub struct IoMethods;

impl Rule for IoMethods {
    const META: RuleMeta = RuleMeta {
        name: "Security/IoMethods",
        department: Department::Security,
        summary: "Checks for the first argument to `IO.read`, `IO.binread`, `IO.write`, `IO.binwrite`, `IO.foreach`, and `IO.readlines`.",
        explanation: "\
Checks for the first argument to `IO.read`, `IO.binread`, `IO.write`, `IO.binwrite`,
`IO.foreach`, and `IO.readlines`.

If argument starts with a pipe character (`'|'`) and the receiver is the `IO` class,
a subprocess is created in the same way as `Kernel#open`, and its output is returned.
`Kernel#open` may allow unintentional command injection, which is the reason these
`IO` methods are a security risk.
Consider using `File.read` to disable the behavior of subprocess invocation.

```ruby
# bad
IO.read(path)
IO.read('path')

# good
File.read(path)
File.read('path')
IO.read('| command') # Allow intentional command invocation.
```",
        enabled_by_default: false,
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
        if !matches!(
            call.name().as_slice(),
            b"read" | b"binread" | b"write" | b"binwrite" | b"foreach" | b"readlines"
        ) {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if !is_io_const(&receiver) {
            return;
        }
        if first_argument_is_pipe_command(&call) {
            return;
        }

        let method_name = String::from_utf8_lossy(call.name().as_slice());
        let message = format!("`File.{method_name}` is safer than `IO.{method_name}`.");
        let span = call_span_excluding_block(&call);
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(receiver.span(), b"File".to_vec())],
            },
        );
    }
}

/// RuboCop's `receiver.source == 'IO'`: a bare `IO` constant reference --
/// not `::IO` or `Foo::IO`, whose source text would differ.
fn is_io_const(node: &Node<'_>) -> bool {
    node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"IO")
}

/// RuboCop's `argument&.str_type? && argument.value.strip.start_with?('|')`.
fn first_argument_is_pipe_command(call: &CallNode<'_>) -> bool {
    let Some(arguments) = call.arguments() else { return false };
    let Some(first) = arguments.arguments().iter().next() else { return false };
    let Some(string) = first.as_string_node() else { return false };
    let bytes = string.unescaped();
    let trimmed = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .map_or(&bytes[bytes.len()..], |i| &bytes[i..]);
    trimmed.starts_with(b"|")
}
