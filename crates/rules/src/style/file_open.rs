//! `Style/FileOpen`, ported from RuboCop's
//! `lib/rubocop/cop/style/file_open.rb`.
//!
//! Upstream's `offensive_usage?` relies on rubocop-ast's generic `Node#receiver`
//! node-matcher (`{(send $_ ...) (any_block (call $_ ...) ...)}`) to test
//! whether the node is specifically the *receiver* of its immediate parent
//! call, which silently returns `nil` (not an offense) for any other parent
//! type instead of raising. Since a Prism `CallNode`'s only directly-nested
//! `Node` child outside of its (wrapped) arguments/block is its own
//! `receiver`, "the immediate parent is a `CallNode`" is exactly that same
//! condition here -- no node-matcher needed.
//!
//! `node.value_used?` (rubocop-ast's general-purpose "is this expression's
//! value used" predicate) is reimplemented in [`value_used`] by climbing
//! `ctx.ancestors()`: whitequark elides a single-statement `begin`/program
//! wrapper entirely (so a lone top-level statement's `parent` is simply
//! `nil`, upstream's unused base case), but Prism always materializes a
//! `StatementsNode`, so the equivalent walk instead climbs past every
//! enclosing `StatementsNode` for which the node (or the `StatementsNode`
//! itself, one level up) is the *last* statement -- matching upstream's
//! `begin_value_used?` -- until it reaches a non-`StatementsNode` ancestor.
//! Reaching the root `ProgramNode` this way is the translation of
//! whitequark's "no parent at all"; reaching anything else (`DefNode`,
//! `BlockNode`, an argument's `ArgumentsNode`, a hash pair's `AssocNode`,
//! an assignment node, ...) is the translation of upstream's `else -> true`
//! fallback, which covers every other parent type this cop's own fixtures
//! exercise.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{ext::const_name, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "`File.open` without a block may leak a file descriptor; use the block form.";

/// RuboCop's `node.value_used?`, reimplemented by climbing past every
/// enclosing `StatementsNode` this node ends up being the last statement
/// of. See the module doc for the full translation rationale.
fn value_used(ctx: &Context<'_>, own_span: Span) -> bool {
    let ancestors = ctx.ancestors();
    let mut end = own_span.end;
    let mut i = ancestors.len();
    loop {
        let Some(index) = i.checked_sub(1) else { return false };
        let ancestor = ancestors[index];
        match ancestor.kind {
            NodeKind::StatementsNode if end == ancestor.span.end => {
                end = ancestor.span.end;
                i = index;
            }
            NodeKind::StatementsNode | NodeKind::ProgramNode => return false,
            _ => return true,
        }
    }
}

/// RuboCop's `offensive_usage?`: the call's value must either be discarded
/// outright or captured in a way upstream considers worth flagging --
/// assigned to a local variable, or itself the receiver of a further
/// chained call.
fn offensive_usage(ctx: &Context<'_>, own_span: Span) -> bool {
    if !value_used(ctx, own_span) {
        return true;
    }
    ctx.parent().is_some_and(|parent| {
        matches!(parent.kind, NodeKind::LocalVariableWriteNode | NodeKind::CallNode)
    })
}

/// Checks for `File.open` without a block, which can leak file descriptors.
#[derive(Debug, Clone)]
pub struct FileOpen;

impl Rule for FileOpen {
    const META: RuleMeta = RuleMeta {
        name: "Style/FileOpen",
        department: Department::Style,
        summary: "Checks for `File.open` without a block, which can leak file descriptors.",
        explanation: "\
When `File.open` is called without a block, the caller is responsible
for closing the file descriptor. If it is not explicitly closed, it
will only be closed when the garbage collector runs, which may lead
to resource exhaustion. Using the block form ensures the file is
automatically closed when the block exits.

This cop only registers an offense when the result of `File.open` is
assigned to a variable or has a method chained on it, as those are the
clearest indicators that the block form should be used instead. When
`File.open` is used as a return value or passed as an argument, the
caller is likely managing the file descriptor intentionally.

@safety
This cop is unsafe because it relies on syntax heuristics and cannot
verify whether the file descriptor is safely managed. For example, it
still flags intentional one-shot reads (`File.open(\"f\").read`) where
the file descriptor is closed by the garbage collector.

```ruby
# bad
f = File.open('file')

# bad
File.open('file').read

# good
File.open('file') do |f|
  f.read
end

# good
File.open('file', &:read)

# good - pass an open file object to an API that manages its lifecycle
process(io: File.open('file'))

# good - return an open file object for the caller to manage
def json_key_io
  File.open('file')
end

# good - use File.read for one-shot reads
File.read('file')
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
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
        if call.is_safe_navigation() || call.name().as_slice() != b"open" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if const_name(&receiver).as_deref() != Some("File") {
            return;
        }
        if call.block().is_some() {
            return;
        }
        if !offensive_usage(ctx, node.span()) {
            return;
        }
        ctx.report(&Self::META, node.span(), MSG);
    }
}
