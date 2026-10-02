//! `Lint/NonAtomicFileOperation`, ported from RuboCop's
//! `lib/rubocop/cop/lint/non_atomic_file_operation.rb`.
//!
//! # Prism shape
//!
//! Whitequark normalizes `unless` into the same `:if` node type as `if`
//! (condition/true-branch/false-branch, swapped for `unless`), so upstream's
//! `parent.if_type?` accepts both; Prism keeps them as separate
//! `IfNode`/`UnlessNode` kinds, both handled here. Whitequark elides a
//! single-statement body (no wrapping `begin`), so `node.parent` for the
//! sole statement of an `if`/`unless` body is the conditional node itself;
//! Prism always wraps a body in a `StatementsNode`, so this walks a
//! hand-rolled ancestor stack (needed for the conditional's own fields, not
//! just its kind) and requires the call to be that `StatementsNode`'s only
//! statement, which is itself the conditional's own `statements` field (not
//! e.g. an `else` branch).
//!
//! `operator_keyword?` (used by `allowable_use_with_if?`) is, per
//! `rubocop-ast`, true for node *type* `:and`/`:or` regardless of whether
//! the source spelled it `&&`/`and` -- whitequark gives `&&` and `and` the
//! same node type. Prism's `AndNode`/`OrNode` are the direct equivalent
//! (also spelling-agnostic), so a compound condition of either kind is
//! treated as "allowable" exactly like upstream.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{ext, walk, LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

/// Upstream's `MSG_REMOVE_FILE_EXIST_CHECK`.
const MSG_REMOVE_FILE_EXIST_CHECK: &str =
    "Remove unnecessary existence check `{receiver}.{method_name}`.";
/// Upstream's `MSG_CHANGE_FORCE_METHOD`.
const MSG_CHANGE_FORCE_METHOD: &str = "Use atomic file operation method `FileUtils.{method_name}`.";

const MAKE_METHODS: &[&[u8]] = &[b"mkdir"];
const MAKE_FORCE_METHODS: &[&[u8]] = &[b"makedirs", b"mkdir_p", b"mkpath"];
const REMOVE_METHODS: &[&[u8]] =
    &[b"remove", b"delete", b"unlink", b"remove_file", b"rm", b"rmdir", b"safe_unlink"];
const RECURSIVE_REMOVE_METHODS: &[&[u8]] =
    &[b"remove_dir", b"remove_entry", b"remove_entry_secure"];
const REMOVE_FORCE_METHODS: &[&[u8]] = &[b"rm_f", b"rm_rf"];
const EXIST_RECEIVERS: &[&[u8]] = &[b"FileTest", b"File", b"Dir", b"Shell"];

/// Checks for non-atomic file operations.
#[derive(Debug, Clone)]
pub struct NonAtomicFileOperation;

impl Rule for NonAtomicFileOperation {
    const META: RuleMeta = RuleMeta {
        name: "Lint/NonAtomicFileOperation",
        department: Department::Lint,
        summary: "Checks for non-atomic file operations.",
        explanation: "\
These can cause problems that are difficult to reproduce,
especially in cases of frequent file operations in parallel,
such as test runs with parallel_rspec.

For examples: creating a directory if there is none, has the following problems

An exception occurs when the directory didn't exist at the time of `exist?`,
but someone else created it before `mkdir` was executed.

Subsequent processes are executed without the directory that should be there
when the directory existed at the time of `exist?`,
but someone else deleted it shortly afterwards.

@safety
  This cop is unsafe, because autocorrection change to atomic processing.
  The atomic processing of the replacement destination is not guaranteed
  to be strictly equivalent to that before the replacement.

```ruby
# bad - race condition with another process may result in an error in `mkdir`
unless Dir.exist?(path)
  FileUtils.mkdir(path)
end

# good - atomic and idempotent creation
FileUtils.mkdir_p(path)

# bad - race condition with another process may result in an error in `remove`
if File.exist?(path)
  FileUtils.remove(path)
end

# good - atomic and idempotent removal
FileUtils.rm_f(path)
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let candidates = {
            let mut finder = Finder { ctx: &*ctx, stack: Vec::new(), candidates: Vec::new() };
            walk(&root, &mut finder);
            finder.candidates
        };
        for candidate in candidates {
            handle_candidate(ctx, &candidate);
        }
    }
}

/// Which conditional kind wraps the call, with enough of its own fields
/// (not just its kind) to build the offense ranges and the fix.
#[derive(Clone, Copy)]
enum Parent<'pr> {
    If(ruby_ast::node::IfNode<'pr>),
    Unless(ruby_ast::node::UnlessNode<'pr>),
}

impl<'pr> Parent<'pr> {
    fn as_node(self) -> Node<'pr> {
        match self {
            Self::If(n) => n.as_node(),
            Self::Unless(n) => n.as_node(),
        }
    }

    fn predicate(self) -> Node<'pr> {
        match self {
            Self::If(n) => n.predicate(),
            Self::Unless(n) => n.predicate(),
        }
    }

    fn statements_span(self) -> Option<Span> {
        match self {
            Self::If(n) => n.statements().map(|s| s.as_node().span()),
            Self::Unless(n) => n.statements().map(|s| s.as_node().span()),
        }
    }

    fn keyword_span(self) -> Span {
        match self {
            Self::If(n) => n.if_keyword_loc().map_or(self.as_node().span(), |l| l.span()),
            Self::Unless(n) => n.keyword_loc().span(),
        }
    }

    fn end_keyword_span(self) -> Option<Span> {
        match self {
            Self::If(n) => n.end_keyword_loc().map(|l| l.span()),
            Self::Unless(n) => n.end_keyword_loc().map(|l| l.span()),
        }
    }

    fn has_else(self) -> bool {
        match self {
            Self::If(n) => n.subsequent().is_some(),
            Self::Unless(n) => n.else_clause().is_some(),
        }
    }

    fn is_elsif(self, ctx: &Context<'_>) -> bool {
        match self {
            Self::If(n) => n.if_keyword_loc().is_some_and(|l| ctx.text(l.span()) == b"elsif"),
            Self::Unless(_) => false,
        }
    }
}

/// One `FileUtils`-style call directly inside an `if`/`unless` body whose
/// condition's first matching existence check shares its first argument.
struct Candidate<'pr> {
    call: CallNode<'pr>,
    parent: Parent<'pr>,
    exist_call: CallNode<'pr>,
}

/// Walks the whole tree, tracking a live ancestor stack (needed for the
/// enclosing conditional's own fields).
struct Finder<'pr, 'c> {
    ctx: &'c Context<'pr>,
    stack: Vec<Node<'pr>>,
    candidates: Vec<Candidate<'pr>>,
}

impl<'pr> Visitor<'pr> for Finder<'pr, '_> {
    fn enter(&mut self, node: &Node<'pr>) {
        if let Some(call) = node.as_call_node() {
            if is_restricted(&call) {
                if let Some(candidate) = check_call(self.ctx, &self.stack, &call) {
                    self.candidates.push(candidate);
                }
            }
        }
        self.stack.push(*node);
    }

    fn leave(&mut self, _node: &Node<'pr>) {
        self.stack.pop();
    }
}

/// Upstream's `RESTRICT_ON_SEND` membership, plus the receiver-is-const
/// check from `on_send`'s own first guard.
fn is_restricted(call: &CallNode<'_>) -> bool {
    let name = call.name();
    let name = name.as_slice();
    MAKE_METHODS.contains(&name)
        || MAKE_FORCE_METHODS.contains(&name)
        || REMOVE_METHODS.contains(&name)
        || RECURSIVE_REMOVE_METHODS.contains(&name)
        || REMOVE_FORCE_METHODS.contains(&name)
}

/// Upstream's `on_send` body (the matching and lookup half; `register_offense`
/// is handled separately once the candidate is known).
fn check_call<'pr>(
    ctx: &Context<'pr>,
    stack: &[Node<'pr>],
    call: &CallNode<'pr>,
) -> Option<Candidate<'pr>> {
    let receiver = call.receiver()?;
    if !ext::is_bare_or_toplevel_const(&receiver) {
        return None;
    }

    let statements = stack.last()?;
    if statements.kind() != NodeKind::StatementsNode {
        return None;
    }
    let stmts = statements.as_statements_node()?;
    if stmts.body().len() != 1 {
        return None;
    }
    let parent_node = stack.get(stack.len().checked_sub(2)?)?;
    let parent = match parent_node.kind() {
        NodeKind::IfNode => Parent::If(parent_node.as_if_node()?),
        NodeKind::UnlessNode => Parent::Unless(parent_node.as_unless_node()?),
        _ => return None,
    };
    if parent.statements_span() != Some(statements.span()) {
        return None;
    }
    if allowable_use_with_if(parent) {
        return None;
    }
    if explicit_not_force(call) {
        return None;
    }
    let exist_call = find_exist_call(&parent.as_node())?;
    // `rubocop-ast`'s `Node#==`: structural equality, approximated here by
    // comparing the two arguments' own source text (both calls' first
    // arguments are simple identifiers/literals in every fixture).
    let a = first_argument_span(call).map(|s| ctx.text(s));
    let b = first_argument_span(&exist_call).map(|s| ctx.text(s));
    if a.is_none() || a != b {
        return None;
    }
    Some(Candidate { call: *call, parent, exist_call })
}

fn first_argument_span(call: &CallNode<'_>) -> Option<Span> {
    call.arguments().and_then(|a| a.arguments().first()).map(|n| n.span())
}

/// Upstream's `allowable_use_with_if?`.
fn allowable_use_with_if(parent: Parent<'_>) -> bool {
    matches!(parent.predicate().kind(), NodeKind::AndNode | NodeKind::OrNode) || parent.has_else()
}

/// Upstream's `explicit_not_force?`: searches `node`'s own subtree for a
/// `force: false` keyword pair.
fn explicit_not_force(call: &CallNode<'_>) -> bool {
    let Some(args) = call.arguments() else { return false };
    args.arguments().iter().any(|arg| has_force_pair(&arg, false))
}

/// Upstream's `force?`/`explicit_not_force?`: a `(pair (sym :force)
/// (:true/:false))` anywhere in `node`'s subtree.
fn has_force_pair(node: &Node<'_>, want: bool) -> bool {
    if let Some(assoc) = node.as_assoc_node() {
        if let Some(key) = assoc.key().as_symbol_node() {
            if key.unescaped() == b"force" {
                let matches = match assoc.value().kind() {
                    NodeKind::TrueNode => want,
                    NodeKind::FalseNode => !want,
                    _ => false,
                };
                if matches {
                    return true;
                }
            }
        }
    }
    let mut found = false;
    ruby_ast::for_each_child(node, |child| {
        found = found || has_force_pair(child, want);
    });
    found
}

/// Upstream's `send_exist_node`: the first (depth-first) call matching
/// `(send (const {cbase nil?} {FileTest File Dir Shell}) {exist? exists?}
/// ...)` anywhere in `node`'s subtree (condition first, then body).
fn find_exist_call<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    if let Some(call) = node.as_call_node() {
        if let Some(receiver) = call.receiver() {
            if let Some(name) = ext::const_name(&receiver) {
                if EXIST_RECEIVERS.contains(&name.as_bytes())
                    && matches!(call.name().as_slice(), b"exist?" | b"exists?")
                {
                    return Some(call);
                }
            }
        }
    }
    let mut found = None;
    ruby_ast::for_each_child(node, |child| {
        if found.is_none() {
            found = find_exist_call(child);
        }
    });
    found
}

/// Upstream's `register_offense`.
fn handle_candidate(ctx: &mut Context<'_>, candidate: &Candidate<'_>) {
    let call = &candidate.call;
    let parent = candidate.parent;
    if !is_force_method(call) {
        let message =
            MSG_CHANGE_FORCE_METHOD.replacen("{method_name}", &replacement_method(call), 1);
        ctx.report(&NonAtomicFileOperation::META, call.as_node().span(), message);
    }

    let range = Span::new(parent.keyword_span().start, parent.predicate().span().end);
    let receiver_name = ext::const_name(&candidate.exist_call.receiver().expect("checked above"))
        .unwrap_or_default();
    let exist_method = String::from_utf8_lossy(candidate.exist_call.name().as_slice()).into_owned();
    let message = MSG_REMOVE_FILE_EXIST_CHECK.replacen("{receiver}", &receiver_name, 1).replacen(
        "{method_name}",
        &exist_method,
        1,
    );

    if parent.is_elsif(ctx) {
        ctx.report(&NonAtomicFileOperation::META, range, message);
    } else {
        let edits = build_fix(call, parent, range);
        let fix = Fix { applicability: Applicability::Unsafe, edits };
        ctx.report_with_fix(&NonAtomicFileOperation::META, range, message, fix);
    }
}

/// Upstream's `autocorrect`.
fn build_fix(call: &CallNode<'_>, parent: Parent<'_>, range: Span) -> Vec<Edit> {
    let mut edits = vec![Edit::delete(range)];
    edits.extend(autocorrect_replace_method(call));
    let modifier_form = parent.end_keyword_span().is_none();
    if modifier_form {
        let gap = Span::new(call.as_node().span().end, parent.keyword_span().start);
        edits.push(Edit::delete(gap));
    } else if let Some(end_span) = parent.end_keyword_span() {
        edits.push(Edit::delete(end_span));
    }
    edits
}

/// The location `rubocop-ast`'s `node.child_nodes.first.loc.name` resolves
/// to for a bare (`FileUtils`) or top-level-qualified (`::FileUtils`)
/// constant receiver: just the identifier itself, excluding a leading `::`.
fn receiver_name_span(receiver: &Node<'_>) -> Option<Span> {
    match receiver.kind() {
        NodeKind::ConstantReadNode => Some(receiver.span()),
        NodeKind::ConstantPathNode => receiver.as_constant_path_node().map(|p| p.name_loc().span()),
        _ => None,
    }
}

/// Upstream's `autocorrect_replace_method`.
fn autocorrect_replace_method(call: &CallNode<'_>) -> Vec<Edit> {
    if is_force_method(call) {
        return Vec::new();
    }
    let mut edits = Vec::new();
    if let Some(receiver) = call.receiver() {
        if let Some(name_loc) = receiver_name_span(&receiver) {
            edits.push(Edit::replace(name_loc, b"FileUtils".to_vec()));
        }
    }
    if let Some(message_loc) = call.message_loc() {
        edits.push(Edit::replace(message_loc.span(), replacement_method(call).into_bytes()));
    }
    if require_mode_keyword(call) {
        if let Some(last) = call.arguments().and_then(|a| a.arguments().last()) {
            edits.push(Edit::insert(last.span().start, b"mode: ".to_vec()));
        }
    }
    edits
}

/// Upstream's `force_method?`.
fn is_force_method(call: &CallNode<'_>) -> bool {
    force_method_name(call) || force_option(call)
}

fn force_method_name(call: &CallNode<'_>) -> bool {
    let name = call.name();
    let name = name.as_slice();
    MAKE_FORCE_METHODS.contains(&name) || REMOVE_FORCE_METHODS.contains(&name)
}

fn force_option(call: &CallNode<'_>) -> bool {
    let Some(args) = call.arguments() else { return false };
    args.arguments().iter().any(|arg| has_force_pair(&arg, true))
}

/// Upstream's `replacement_method`.
fn replacement_method(call: &CallNode<'_>) -> String {
    let name = call.name();
    let name = name.as_slice();
    if MAKE_METHODS.contains(&name) {
        "mkdir_p".to_string()
    } else if REMOVE_METHODS.contains(&name) {
        "rm_f".to_string()
    } else if RECURSIVE_REMOVE_METHODS.contains(&name) {
        "rm_rf".to_string()
    } else {
        String::from_utf8_lossy(name).into_owned()
    }
}

/// Upstream's `require_mode_keyword?`.
fn require_mode_keyword(call: &CallNode<'_>) -> bool {
    let Some(receiver) = call.receiver() else { return false };
    if ext::const_name(&receiver).as_deref() != Some("Dir") {
        return false;
    }
    replacement_method(call) == "mkdir_p"
        && call.arguments().is_some_and(|a| a.arguments().len() == 2)
}
