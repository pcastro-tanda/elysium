//! `Lint/IncompatibleIoSelectWithFiberScheduler`, ported from RuboCop's
//! `lib/rubocop/cop/lint/incompatible_io_select_with_fiber_scheduler.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{node::ArgumentsNode, Node};
use ruby_ast::{NodeExt as _, NodeKind};

/// Whether `node` is a literal `nil` or a literal empty array (`[]`) --
/// RuboCop's `(io2.nil? || io2.nil_type?)` and the empty-array leg of
/// `io2&.array_type? ? io2.values.empty? : ...` share this shape.
fn is_empty_or_nil(node: &Node<'_>) -> bool {
    if node.as_nil_node().is_some() {
        return true;
    }
    if let Some(array) = node.as_array_node() {
        return array.elements().iter().next().is_none();
    }
    false
}

/// RuboCop's `single_io_array?`: a literal array with exactly one element
/// that is not itself a splat.
fn single_io_array<'pr>(node: Option<&Node<'pr>>) -> Option<Node<'pr>> {
    let array = node?.as_array_node()?;
    let elements: Vec<Node<'pr>> = array.elements().iter().collect();
    let [element] = elements.as_slice() else { return None };
    if element.as_splat_node().is_some() {
        return None;
    }
    Some(*element)
}

/// RuboCop's `scheduler_compatible?`: `io1` must be a single-element array,
/// and `io2` must be absent, a literal `nil`, or a literal empty array.
fn scheduler_compatible(io1: Option<&Node<'_>>, io2: Option<&Node<'_>>) -> bool {
    if single_io_array(io1).is_none() {
        return false;
    }
    match io2 {
        None => true,
        Some(node) => is_empty_or_nil(node),
    }
}

/// RuboCop's `node.parent&.assignment?`: whether `node`'s parent is one of
/// the assignment node types (including multiple assignment), in which case
/// autocorrection is skipped because the replacement's return value differs
/// from `IO.select`'s.
fn parent_is_assignment(ctx: &Context<'_>) -> bool {
    let Some(parent) = ctx.parent() else { return false };
    matches!(
        parent.kind,
        NodeKind::LocalVariableWriteNode
            | NodeKind::LocalVariableAndWriteNode
            | NodeKind::LocalVariableOrWriteNode
            | NodeKind::LocalVariableOperatorWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::InstanceVariableAndWriteNode
            | NodeKind::InstanceVariableOrWriteNode
            | NodeKind::InstanceVariableOperatorWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::ClassVariableAndWriteNode
            | NodeKind::ClassVariableOrWriteNode
            | NodeKind::ClassVariableOperatorWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::GlobalVariableOperatorWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantAndWriteNode
            | NodeKind::ConstantOrWriteNode
            | NodeKind::ConstantOperatorWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::ConstantPathAndWriteNode
            | NodeKind::ConstantPathOrWriteNode
            | NodeKind::ConstantPathOperatorWriteNode
            | NodeKind::CallAndWriteNode
            | NodeKind::CallOrWriteNode
            | NodeKind::CallOperatorWriteNode
            | NodeKind::IndexAndWriteNode
            | NodeKind::IndexOrWriteNode
            | NodeKind::IndexOperatorWriteNode
            | NodeKind::MultiWriteNode
    )
}

/// Checks for `IO.select` that is incompatible with Fiber Scheduler.
#[derive(Debug, Clone)]
pub struct IncompatibleIoSelectWithFiberScheduler;

impl IncompatibleIoSelectWithFiberScheduler {
    /// RuboCop's `*io_select(node)`: positionally destructure the call's
    /// arguments into `(read, write, excepts, timeout)`, each `None` when
    /// not given.
    fn positional_args<'pr>(arguments: &ArgumentsNode<'pr>) -> [Option<Node<'pr>>; 4] {
        let args: Vec<Node<'pr>> = arguments.arguments().iter().collect();
        let mut out: [Option<Node<'pr>>; 4] = [None, None, None, None];
        for (slot, arg) in out.iter_mut().zip(args) {
            *slot = Some(arg);
        }
        out
    }
}

impl Rule for IncompatibleIoSelectWithFiberScheduler {
    const META: RuleMeta = RuleMeta {
        name: "Lint/IncompatibleIoSelectWithFiberScheduler",
        department: Department::Lint,
        summary: "Checks for `IO.select` that is incompatible with Fiber Scheduler.",
        explanation: "\
Checks for `IO.select` that is incompatible with Fiber Scheduler since Ruby 3.0.

When an array of IO objects waiting for an exception (the third argument of
`IO.select`) is used as an argument, there is no alternative API, so offenses
are not registered.

```ruby
# bad
IO.select([io], [], [], timeout)

# good
io.wait_readable(timeout)

# bad
IO.select([], [io], [], timeout)

# good
io.wait_writable(timeout)
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
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
        if call.name().as_slice() != b"select" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let is_io = if let Some(c) = receiver.as_constant_read_node() {
            c.name().as_slice() == b"IO"
        } else if let Some(path) = receiver.as_constant_path_node() {
            path.parent().is_none() && path.name().is_some_and(|n| n.as_slice() == b"IO")
        } else {
            false
        };
        if !is_io {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let [read, write, excepts, timeout] = Self::positional_args(&arguments);

        if let Some(excepts) = &excepts {
            if !is_empty_or_nil(excepts) {
                return;
            }
        }

        if !scheduler_compatible(read.as_ref(), write.as_ref())
            && !scheduler_compatible(write.as_ref(), read.as_ref())
        {
            return;
        }

        let timeout_argument = match &timeout {
            None => String::new(),
            Some(t) => format!("({})", String::from_utf8_lossy(ctx.text(t.span()))),
        };

        let preferred = if let Some(read_first) =
            read.as_ref().and_then(Node::as_array_node).and_then(|a| a.elements().iter().next())
        {
            format!(
                "{}.wait_readable{timeout_argument}",
                String::from_utf8_lossy(ctx.text(read_first.span()))
            )
        } else {
            let write_first = write
                .as_ref()
                .and_then(Node::as_array_node)
                .and_then(|a| a.elements().iter().next())
                .expect("scheduler_compatible requires a single-element write array");
            format!(
                "{}.wait_writable{timeout_argument}",
                String::from_utf8_lossy(ctx.text(write_first.span()))
            )
        };

        let span = node.span();
        let current = String::from_utf8_lossy(ctx.text(span));
        let message = format!("Use `{preferred}` instead of `{current}`.");

        if parent_is_assignment(ctx) {
            ctx.report(&Self::META, span, message);
            return;
        }

        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(span, preferred.into_bytes())],
        };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }
}
