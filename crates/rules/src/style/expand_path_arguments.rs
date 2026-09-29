//! `Style/ExpandPathArguments`, ported from RuboCop's
//! `lib/rubocop/cop/style/expand_path_arguments.rb`.
//!
//! Three independent shapes, all reached through a `CallNode` named
//! `expand_path` (upstream's `RESTRICT_ON_SEND`):
//!
//! - `File.expand_path(current_path, __FILE__)` / `::File.expand_path(...)`
//!   ([`file_expand_path`]): the two-argument form, offense anchored at the
//!   `expand_path` selector (`node.loc.selector`).
//! - `Pathname(__FILE__).parent.expand_path` ([`pathname_parent_expand_path`])
//!   and `Pathname.new(__FILE__).parent.expand_path` /
//!   `::Pathname.new(__FILE__).parent.expand_path`
//!   ([`pathname_new_parent_expand_path`]): fixed messages, offense spanning
//!   the whole chain.
//!
//! Every shape requires a plain `(send ...)` at each link -- a `&.` anywhere
//! in the chain (safe navigation) does not match upstream's node patterns,
//! so [`CallNode::is_safe_navigation`] is checked at every link that could
//! carry one.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `PATHNAME_MSG`.
const PATHNAME_MSG: &str = "Use `Pathname(__dir__).expand_path` instead of \
`Pathname(__FILE__).parent.expand_path`.";
/// RuboCop's `PATHNAME_NEW_MSG`.
const PATHNAME_NEW_MSG: &str = "Use `Pathname.new(__dir__).expand_path` instead of \
`Pathname.new(__FILE__).parent.expand_path`.";

/// RuboCop's `unrecommended_argument?`: the node's source text is exactly
/// `__FILE__`.
fn is_unrecommended_argument(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    ctx.text(node.span()) == b"__FILE__"
}

/// RuboCop's `depth`: the number of non-`.` path components.
fn depth(path: &str) -> usize {
    path.split('/').filter(|part| *part != ".").count()
}

/// RuboCop's `parent_path`: every `.` component dropped, then the first
/// `..` component (only the first) dropped too, rejoined with `/`.
fn parent_path(path: &str) -> String {
    let mut parts: Vec<&str> = path.split('/').filter(|part| *part != ".").collect();
    if let Some(index) = parts.iter().position(|part| *part == "..") {
        parts.remove(index);
    }
    parts.join("/")
}

/// RuboCop's `file_expand_path` node matcher: `File.expand_path(current_path,
/// default_dir)` / `::File.expand_path(...)`, a call named `expand_path` on
/// a bare or top-level-qualified `File` constant with exactly two positional
/// arguments.
fn file_expand_path<'pr>(call: &CallNode<'pr>) -> Option<(Node<'pr>, Node<'pr>)> {
    let receiver = call.receiver()?;
    if !ext::is_bare_or_toplevel_const(&receiver)
        || ext::const_name(&receiver).as_deref() != Some("File")
    {
        return None;
    }
    let args = call.arguments()?.arguments();
    if args.len() != 2 {
        return None;
    }
    let mut iter = args.iter();
    let current_path = iter.next()?;
    let default_dir = iter.next()?;
    Some((current_path, default_dir))
}

/// The `.parent` call directly under `call` (its receiver), or `None` when
/// that shape does not hold: a receiver-less, argument-less, non-safe-nav
/// call literally named `parent`.
fn parent_receiver<'pr>(call: &CallNode<'pr>) -> Option<CallNode<'pr>> {
    if call.arguments().is_some() {
        return None;
    }
    let parent_call = call.receiver()?.as_call_node()?;
    if parent_call.is_safe_navigation()
        || parent_call.arguments().is_some()
        || parent_call.name().as_slice() != b"parent"
    {
        return None;
    }
    Some(parent_call)
}

/// RuboCop's `pathname_parent_expand_path` node matcher:
/// `Pathname(default_dir).parent.expand_path`. Returns the `default_dir`
/// argument.
fn pathname_parent_expand_path<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let parent_call = parent_receiver(call)?;
    let inner = parent_call.receiver()?.as_call_node()?;
    if inner.is_safe_navigation()
        || inner.receiver().is_some()
        || inner.name().as_slice() != b"Pathname"
    {
        return None;
    }
    let args = inner.arguments()?.arguments();
    if args.len() != 1 {
        return None;
    }
    args.first()
}

/// RuboCop's `pathname_new_parent_expand_path` node matcher:
/// `Pathname.new(default_dir).parent.expand_path` /
/// `::Pathname.new(default_dir).parent.expand_path`. Returns the
/// `default_dir` argument.
fn pathname_new_parent_expand_path<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let parent_call = parent_receiver(call)?;
    let new_call = parent_call.receiver()?.as_call_node()?;
    if new_call.is_safe_navigation() || new_call.name().as_slice() != b"new" {
        return None;
    }
    let receiver = new_call.receiver()?;
    if !ext::is_bare_or_toplevel_const(&receiver)
        || ext::const_name(&receiver).as_deref() != Some("Pathname")
    {
        return None;
    }
    let args = new_call.arguments()?.arguments();
    if args.len() != 1 {
        return None;
    }
    args.first()
}

/// Use `expand_path(__dir__)` instead of `expand_path('..', __FILE__)`.
#[derive(Debug, Clone)]
pub struct ExpandPathArguments;

impl ExpandPathArguments {
    /// RuboCop's `inspect_offense_for_expand_path`: builds the interpolated
    /// message and, via [`Self::autocorrect_expand_path`], the fix.
    fn inspect_file_expand_path(
        call: &CallNode<'_>,
        current_path: Node<'_>,
        default_dir: &Node<'_>,
        ctx: &mut Context<'_>,
    ) {
        if !is_unrecommended_argument(default_dir, ctx) {
            return;
        }
        let Some(str_node) = current_path.as_string_node() else { return };
        let Some(selector) = call.message_loc() else { return };

        let stripped = String::from_utf8_lossy(str_node.unescaped()).into_owned();
        let parent = parent_path(&stripped);
        let depth = depth(&stripped);
        let new_path = if parent.is_empty() { String::new() } else { format!("'{parent}', ") };
        let new_default_dir = if depth == 0 { "__FILE__" } else { "__dir__" };
        let message = format!(
            "Use `expand_path({new_path}{new_default_dir})` instead of \
             `expand_path('{stripped}', __FILE__)`."
        );

        let arg_range = Span::new(current_path.span().start, default_dir.span().end);
        let edits = match depth {
            0 => vec![Edit::replace(arg_range, b"__FILE__".to_vec())],
            1 => vec![Edit::replace(arg_range, b"__dir__".to_vec())],
            _ => vec![
                Edit::replace(current_path.span(), format!("'{parent}'").into_bytes()),
                Edit::replace(default_dir.span(), b"__dir__".to_vec()),
            ],
        };
        let fix = Fix { applicability: Applicability::Safe, edits };
        ctx.report_with_fix(&Self::META, selector.span(), message, fix);
    }

    /// RuboCop's shared `add_offense`/`autocorrect` path for both Pathname
    /// shapes: replaces `default_dir` with `__dir__` and removes the
    /// `.parent` call feeding into `call`.
    fn inspect_pathname(
        call: &CallNode<'_>,
        default_dir: &Node<'_>,
        message: &'static str,
        ctx: &mut Context<'_>,
    ) {
        if !is_unrecommended_argument(default_dir, ctx) {
            return;
        }
        let Some(parent_call) = call.receiver().and_then(|r| r.as_call_node()) else { return };

        let mut edits = vec![Edit::replace(default_dir.span(), b"__dir__".to_vec())];
        if let Some(dot) = parent_call.call_operator_loc() {
            edits.push(Edit::delete(dot.span()));
        }
        if let Some(selector) = parent_call.message_loc() {
            edits.push(Edit::delete(selector.span()));
        }
        let fix = Fix { applicability: Applicability::Safe, edits };
        ctx.report_with_fix(&Self::META, call.as_node().span(), message, fix);
    }
}

impl Rule for ExpandPathArguments {
    const META: RuleMeta = RuleMeta {
        name: "Style/ExpandPathArguments",
        department: Department::Style,
        summary: "Use `expand_path(__dir__)` instead of `expand_path('..', __FILE__)`.",
        explanation: "\
Checks for use of the `File.expand_path` arguments.
Likewise, it also checks for the `Pathname.new` argument.

```ruby
# bad
File.expand_path('..', __FILE__)

# good
File.expand_path(__dir__)

# bad
Pathname.new(__FILE__).parent.expand_path

# good
Pathname.new(__dir__).expand_path
```",
        enabled_by_default: true,
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
        if call.is_safe_navigation() || call.name().as_slice() != b"expand_path" {
            return;
        }

        if let Some((current_path, default_dir)) = file_expand_path(&call) {
            Self::inspect_file_expand_path(&call, current_path, &default_dir, ctx);
        } else if let Some(default_dir) = pathname_parent_expand_path(&call) {
            Self::inspect_pathname(&call, &default_dir, PATHNAME_MSG, ctx);
        } else if let Some(default_dir) = pathname_new_parent_expand_path(&call) {
            Self::inspect_pathname(&call, &default_dir, PATHNAME_NEW_MSG, ctx);
        }
    }
}
