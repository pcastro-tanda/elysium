//! `Style/EvalWithLocation`, ported from RuboCop's
//! `lib/rubocop/cop/style/eval_with_location.rb`.
//!
//! # `__FILE__`/`__LINE__` are dedicated node kinds
//!
//! Whitequark parses `__FILE__` as a `str` node whose source text happens to
//! be the literal keyword, and `__LINE__` as an `int` node likewise; that is
//! why upstream's `special_file_keyword?`/`special_line_keyword?` compare
//! `node.source` against those keyword strings. Prism instead gives each its
//! own dedicated node kind -- `SourceFileNode`/`SourceLineNode` -- so both
//! checks here collapse to a `NodeKind` comparison, no source text involved.
//!
//! Correspondingly, upstream's `line_with_offset?` node pattern
//! `(send #special_line_keyword? %1 (int %2))` (or the operands swapped)
//! becomes: a `CallNode` whose receiver or sole argument is a
//! `SourceLineNode`, the other slot an `IntegerNode` equal to the expected
//! offset, and the method name the expected sign.
//!
//! # Only `+`-shaped line arguments are ever validated
//!
//! Upstream's `check_line` bails out before computing anything whenever the
//! last argument is a variable, or is a `send` node whose method is not
//! `:+` -- which includes `__LINE__ - 3` and any other call shape. This is
//! not a whitequark/Prism quirk: it means a `__LINE__ - N` line argument is
//! *never* validated for correctness by this cop upstream, and this port
//! preserves that literally rather than "fixing" it to also validate minus
//! forms.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const, is_heredoc};
use ruby_ast::node::{ArgumentsNode, CallNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// Pass `__FILE__` and `__LINE__` to `eval` method, as they are used by backtraces.
#[derive(Debug, Clone)]
pub struct EvalWithLocation;

/// `rubocop-ast`'s `VARIABLES` set (`Node#variable?`): local, instance,
/// class and global variable reads.
fn is_variable_read(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::LocalVariableReadNode
            | NodeKind::InstanceVariableReadNode
            | NodeKind::ClassVariableReadNode
            | NodeKind::GlobalVariableReadNode
    )
}

/// RuboCop's `valid_eval_receiver?`: no receiver, or a bare/top-level-
/// qualified `Kernel` constant.
fn valid_eval_receiver(receiver: Option<Node<'_>>) -> bool {
    match receiver {
        None => true,
        Some(r) => is_bare_or_toplevel_const(&r) && const_name(&r).as_deref() == Some("Kernel"),
    }
}

/// RuboCop's `string_first_line`: a heredoc's body's first line (its
/// content start, Prism's stand-in for the unmodelled `loc.heredoc_body`),
/// or the node's own first line otherwise.
fn string_first_line(code: &Node<'_>, ctx: &Context<'_>) -> u32 {
    let content_start = if is_heredoc(code) {
        code.as_string_node().map(|n| n.content_loc().span().start).or_else(|| {
            code.as_interpolated_string_node()
                .and_then(|n| n.parts().first())
                .map(|p| p.span().start)
        })
    } else {
        None
    };
    ctx.line_col(content_start.unwrap_or_else(|| code.span().start)).line
}

/// RuboCop's `line_difference`.
fn line_difference(line_node: &Node<'_>, code: &Node<'_>, ctx: &Context<'_>) -> i64 {
    i64::from(string_first_line(code, ctx)) - i64::from(ctx.line_col(line_node.span().start).line)
}

/// RuboCop's `expected_line`.
fn expected_line(diff: i64) -> String {
    if diff == 0 {
        "__LINE__".to_string()
    } else {
        format!("__LINE__ {} {}", if diff > 0 { '+' } else { '-' }, diff.abs())
    }
}

/// RuboCop's `line_with_offset?`: `line_node` is already exactly
/// `__LINE__ <sign> <num>` (in either operand order).
fn line_with_offset(line_node: &Node<'_>, sign: u8, num: u32) -> bool {
    let Some(call) = line_node.as_call_node() else { return false };
    if call.name().as_slice() != [sign] {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    let Some(args) = call.arguments() else { return false };
    let list = args.arguments();
    if list.len() != 1 {
        return false;
    }
    let arg = list.first().expect("length checked");
    let matches_num = |int_node: Node<'_>| {
        int_node
            .as_integer_node()
            .is_some_and(|i| TryInto::<i32>::try_into(i.value()).ok() == i32::try_from(num).ok())
    };
    if receiver.kind() == NodeKind::SourceLineNode {
        matches_num(arg)
    } else if arg.kind() == NodeKind::SourceLineNode {
        matches_num(receiver)
    } else {
        false
    }
}

impl EvalWithLocation {
    /// RuboCop's `on_send`.
    fn check(call: &CallNode<'_>, ctx: &mut Context<'_>) {
        if call.is_safe_navigation() {
            return;
        }
        let name = call.name();
        let name_bytes = name.as_slice();
        let is_eval = name_bytes == b"eval";
        if !is_eval && !matches!(name_bytes, b"class_eval" | b"module_eval" | b"instance_eval") {
            return;
        }
        if is_eval && !valid_eval_receiver(call.receiver()) {
            return;
        }
        let Some(args) = call.arguments() else { return };
        let Some(code) = args.arguments().first() else { return };
        if !matches!(code.kind(), NodeKind::StringNode | NodeKind::InterpolatedStringNode) {
            return;
        }

        let base = if is_eval { 2 } else { 1 };
        let file = args.arguments().iter().nth(base);
        let line = args.arguments().iter().nth(base + 1);

        match (file, line) {
            (Some(file), Some(line)) => {
                Self::check_file(&file, name_bytes, ctx);
                Self::check_line(&line, &code, name_bytes, ctx);
            }
            (Some(file), None) => {
                Self::check_file(&file, name_bytes, ctx);
                Self::add_offense_for_missing_line(call, &args, &code, is_eval, name_bytes, ctx);
            }
            (None, _) => {
                Self::add_offense_for_missing_location(
                    call, &args, &code, is_eval, name_bytes, ctx,
                );
            }
        }
    }

    /// RuboCop's `check_file`.
    fn check_file(file: &Node<'_>, name_bytes: &[u8], ctx: &mut Context<'_>) {
        if file.kind() == NodeKind::SourceFileNode {
            return;
        }
        let span = file.span();
        let actual = String::from_utf8_lossy(ctx.text(span)).into_owned();
        let method = String::from_utf8_lossy(name_bytes).into_owned();
        let message =
            format!("Incorrect file for `{method}`; use `__FILE__` instead of `{actual}`.");
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(span, b"__FILE__".to_vec())],
        };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }

    /// RuboCop's `check_line`.
    fn check_line(line_node: &Node<'_>, code: &Node<'_>, name_bytes: &[u8], ctx: &mut Context<'_>) {
        if is_variable_read(line_node) {
            return;
        }
        if let Some(call) = line_node.as_call_node() {
            if call.name().as_slice() != b"+" {
                return;
            }
        }
        let diff = line_difference(line_node, code, ctx);
        if diff == 0 {
            if line_node.kind() == NodeKind::SourceLineNode {
                return;
            }
        } else {
            let sign = if diff > 0 { b'+' } else { b'-' };
            if line_with_offset(line_node, sign, u32::try_from(diff.unsigned_abs()).unwrap_or(0)) {
                return;
            }
        }
        let expected = expected_line(diff);
        let span = line_node.span();
        let actual = String::from_utf8_lossy(ctx.text(span)).into_owned();
        let method = String::from_utf8_lossy(name_bytes).into_owned();
        let message = format!(
            "Incorrect line number for `{method}`; use `{expected}` instead of `{actual}`."
        );
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(span, expected.into_bytes())],
        };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }

    /// RuboCop's `register_offense`'s message.
    fn missing_message(is_eval: bool, name_bytes: &[u8]) -> String {
        if is_eval {
            "Pass a binding, `__FILE__`, and `__LINE__` to `eval`.".to_string()
        } else {
            let method = String::from_utf8_lossy(name_bytes).into_owned();
            format!("Pass `__FILE__` and `__LINE__` to `{method}`.")
        }
    }

    /// RuboCop's `add_offense_for_missing_line`.
    fn add_offense_for_missing_line(
        call: &CallNode<'_>,
        args: &ArgumentsNode<'_>,
        code: &Node<'_>,
        is_eval: bool,
        name_bytes: &[u8],
        ctx: &mut Context<'_>,
    ) {
        let message = Self::missing_message(is_eval, name_bytes);
        let last = args.arguments().last().expect("file argument present");
        let diff = line_difference(&last, code, ctx);
        let line_str = expected_line(diff);
        let insert_at = last.span().end;
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::insert(insert_at, format!(", {line_str}").into_bytes())],
        };
        ctx.report_with_fix(&Self::META, call.as_node().span(), message, fix);
    }

    /// RuboCop's `add_offense_for_missing_location`.
    fn add_offense_for_missing_location(
        call: &CallNode<'_>,
        args: &ArgumentsNode<'_>,
        code: &Node<'_>,
        is_eval: bool,
        name_bytes: &[u8],
        ctx: &mut Context<'_>,
    ) {
        let message = Self::missing_message(is_eval, name_bytes);
        // RuboCop's `with_binding?`: for bare `eval`, at least 2 arguments
        // (code + binding) must already be present, or no fix is offered.
        if is_eval && args.arguments().len() < 2 {
            ctx.report(&Self::META, call.as_node().span(), message);
            return;
        }
        let last = args.arguments().last().expect("code argument present");
        let diff = line_difference(&last, code, ctx);
        let line_str = expected_line(diff);
        let insert_at = last.span().end;
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::insert(insert_at, format!(", __FILE__, {line_str}").into_bytes())],
        };
        ctx.report_with_fix(&Self::META, call.as_node().span(), message, fix);
    }
}

impl Rule for EvalWithLocation {
    const META: RuleMeta = RuleMeta {
        name: "Style/EvalWithLocation",
        department: Department::Style,
        summary: "Pass `__FILE__` and `__LINE__` to `eval` method, as they are used by backtraces.",
        explanation: "\
Ensures that eval methods (`eval`, `instance_eval`, `class_eval` and
`module_eval`) are given filename and line number values (`__FILE__` and
`__LINE__`). This data is used to ensure that any errors raised within the
evaluated code will be given the correct identification in a backtrace.

The cop also checks that the line number given relative to `__LINE__` is
correct.

This cop will autocorrect incorrect or missing filename and line number
values. However, if `eval` is called without a binding argument, the cop
will not attempt to automatically add a binding, or add filename and line
values.

NOTE: This cop works only when a string literal is given as a code string.
No offense is reported if a string variable is given as below:

```ruby
code = <<-RUBY
  def do_something
  end
RUBY
eval code # not checked.
```

```ruby
# bad
eval <<-RUBY
  def do_something
  end
RUBY

# bad
C.class_eval <<-RUBY
  def do_something
  end
RUBY

# good
eval <<-RUBY, binding, __FILE__, __LINE__ + 1
  def do_something
  end
RUBY

# good
C.class_eval <<-RUBY, __FILE__, __LINE__ + 1
  def do_something
  end
RUBY
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Upstream's `check_line` only ever validates a line argument shaped as a bare
`__LINE__` or a `__LINE__ + N` call; any other call shape (notably
`__LINE__ - N`) is accepted unconditionally, correct or not. This port
preserves that literally rather than also validating minus forms.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        Self::check(&call, ctx);
    }
}
