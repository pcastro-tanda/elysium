//! `Lint/ShadowedException`, ported from RuboCop's
//! `lib/rubocop/cop/lint/shadowed_exception.rb` plus the `RescueNode` mixin
//! it includes (`lib/rubocop/cop/mixin/rescue_node.rb`, whose only surviving
//! contribution here is the modifier-form guard: in whitequark, `foo rescue
//! nil` and `begin; foo; rescue; nil; end` both parse to a `:rescue` node,
//! so RuboCop must inspect token positions to tell them apart. Prism gives
//! the modifier form its own node kind (`RescueModifierNode`), so this rule
//! simply never subscribes to it).
//!
//! RuboCop's whitequark-based `rescue` node wraps a list of `resbody`
//! children (`node.resbody_branches`); Prism instead links each `rescue`
//! clause to the next with `RescueNode::subsequent`, and folds a `def`'s
//! implicit `rescue`/`ensure` into a `BeginNode` body exactly like an
//! explicit `begin...end` (verified against `Prism.parse`: `DefNode#body`
//! is a `BeginNode` even with no `begin` keyword in the source), so
//! subscribing to `NodeKind::BeginNode` and reading `rescue_clause` covers
//! both `RuboCop::Cop::Lint::ShadowedException#on_rescue` callsites (a
//! `begin` node and a `def` node both dispatch there in whitechalk).
//!
//! `evaluate_exceptions` mirrors `Kernel.const_get(exception.source)` by
//! matching the exception expression's raw source text against
//! [`EXCEPTION_HIERARCHY`] (every built-in class such that `c < Exception`,
//! generated from a real Ruby 3.4 process, see the table's own doc comment)
//! instead of actually resolving constants: any source text that is not one
//! of those exact dotted names -- a custom class, a splat (`*FOO`), a bare
//! method call, or an explicit array literal (`[bar]`, structurally
//! indistinguishable from a comma list at this level, same as upstream) --
//! becomes `None`, matching the `rescue NameError; converted << nil` branch.
//!
//! `compare_exceptions`'s `system_call_err?` branch special-cases pairs of
//! `Errno::*` classes (`ancestors[1] == SystemCallError`, i.e. an exact
//! immediate subclass of `SystemCallError`): real Ruby's `Module#<=>`
//! between two distinct sibling classes is always `nil`, and between the
//! same class is `0`, so `errno.const_get(:Errno) != other.const_get(:Errno)
//! && (exception <=> other_exception)` can only ever be `nil && 0` (same
//! class, so its own `Errno` constant compares equal to itself: `false && _`
//! short-circuits) or `true && nil` (distinct sibling classes): both falsy.
//! `is_system_call_err` therefore always returns `false` for any two
//! `Errno::*` names without needing real per-platform error codes.
//!
//! `offense_range` cannot reuse `RescueNode::location`: unlike whitequark's
//! `resbody` (which never sees its siblings), Prism's `RescueNode` location
//! for a non-last clause in the chain extends through every `subsequent`
//! clause too (verified against `Prism.parse`: the first of two chained
//! `rescue` clauses reports the same `end_offset` as the second). RuboCop's
//! `shadowing_rescue.source_range` is only ever the shadowing resbody's own
//! exceptions/reference/body, so [`shadowing_span`] recomputes it from the
//! `keyword_loc` through the end of the clause's own last present child.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::RescueNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;
use std::cmp::Ordering;

/// RuboCop's `MSG`.
const MSG: &str = "Do not shadow rescued Exceptions.";

/// Every built-in class `c` such that `c < Exception`, as `(c.to_s,
/// c.superclass.to_s)`, generated on Ruby 3.4.2 by:
///
/// ```sh
/// ruby -e 'ObjectSpace.each_object(Class).select{|c| c < Exception}.each{|c| puts "#{c} < #{c.superclass}"}'
/// ```
///
/// (`RubyGems`, `error_highlight`, and the other default gems it pulls in
/// are always loaded by a real `ruby` process the way RuboCop itself runs,
/// so their exception classes are included too -- `Kernel.const_get` would
/// resolve them in the real cop just as readily as `StandardError`.) Sorted
/// alphabetically by class name; order is otherwise irrelevant, lookups are
/// linear scans over a small, `const`-evaluable table.
#[rustfmt::skip]
const EXCEPTION_HIERARCHY: &[(&str, &str)] = &[
    ("ArgumentError", "StandardError"),
    ("ClosedQueueError", "StopIteration"),
    ("EOFError", "IOError"),
    ("Encoding::CompatibilityError", "EncodingError"),
    ("Encoding::ConverterNotFoundError", "EncodingError"),
    ("Encoding::InvalidByteSequenceError", "EncodingError"),
    ("Encoding::UndefinedConversionError", "EncodingError"),
    ("EncodingError", "StandardError"),
    ("Errno::E2BIG", "SystemCallError"),
    ("Errno::EACCES", "SystemCallError"),
    ("Errno::EADDRINUSE", "SystemCallError"),
    ("Errno::EADDRNOTAVAIL", "SystemCallError"),
    ("Errno::EAFNOSUPPORT", "SystemCallError"),
    ("Errno::EAGAIN", "SystemCallError"),
    ("Errno::EALREADY", "SystemCallError"),
    ("Errno::EAUTH", "SystemCallError"),
    ("Errno::EBADARCH", "SystemCallError"),
    ("Errno::EBADEXEC", "SystemCallError"),
    ("Errno::EBADF", "SystemCallError"),
    ("Errno::EBADMACHO", "SystemCallError"),
    ("Errno::EBADMSG", "SystemCallError"),
    ("Errno::EBADRPC", "SystemCallError"),
    ("Errno::EBUSY", "SystemCallError"),
    ("Errno::ECANCELED", "SystemCallError"),
    ("Errno::ECHILD", "SystemCallError"),
    ("Errno::ECONNABORTED", "SystemCallError"),
    ("Errno::ECONNREFUSED", "SystemCallError"),
    ("Errno::ECONNRESET", "SystemCallError"),
    ("Errno::EDEADLK", "SystemCallError"),
    ("Errno::EDESTADDRREQ", "SystemCallError"),
    ("Errno::EDEVERR", "SystemCallError"),
    ("Errno::EDOM", "SystemCallError"),
    ("Errno::EDQUOT", "SystemCallError"),
    ("Errno::EEXIST", "SystemCallError"),
    ("Errno::EFAULT", "SystemCallError"),
    ("Errno::EFBIG", "SystemCallError"),
    ("Errno::EFTYPE", "SystemCallError"),
    ("Errno::EHOSTDOWN", "SystemCallError"),
    ("Errno::EHOSTUNREACH", "SystemCallError"),
    ("Errno::EIDRM", "SystemCallError"),
    ("Errno::EILSEQ", "SystemCallError"),
    ("Errno::EINPROGRESS", "SystemCallError"),
    ("Errno::EINTR", "SystemCallError"),
    ("Errno::EINVAL", "SystemCallError"),
    ("Errno::EIO", "SystemCallError"),
    ("Errno::EISCONN", "SystemCallError"),
    ("Errno::EISDIR", "SystemCallError"),
    ("Errno::ELOOP", "SystemCallError"),
    ("Errno::EMFILE", "SystemCallError"),
    ("Errno::EMLINK", "SystemCallError"),
    ("Errno::EMSGSIZE", "SystemCallError"),
    ("Errno::EMULTIHOP", "SystemCallError"),
    ("Errno::ENAMETOOLONG", "SystemCallError"),
    ("Errno::ENEEDAUTH", "SystemCallError"),
    ("Errno::ENETDOWN", "SystemCallError"),
    ("Errno::ENETRESET", "SystemCallError"),
    ("Errno::ENETUNREACH", "SystemCallError"),
    ("Errno::ENFILE", "SystemCallError"),
    ("Errno::ENOATTR", "SystemCallError"),
    ("Errno::ENOBUFS", "SystemCallError"),
    ("Errno::ENODATA", "SystemCallError"),
    ("Errno::ENODEV", "SystemCallError"),
    ("Errno::ENOENT", "SystemCallError"),
    ("Errno::ENOEXEC", "SystemCallError"),
    ("Errno::ENOLCK", "SystemCallError"),
    ("Errno::ENOLINK", "SystemCallError"),
    ("Errno::ENOMEM", "SystemCallError"),
    ("Errno::ENOMSG", "SystemCallError"),
    ("Errno::ENOPOLICY", "SystemCallError"),
    ("Errno::ENOPROTOOPT", "SystemCallError"),
    ("Errno::ENOSPC", "SystemCallError"),
    ("Errno::ENOSR", "SystemCallError"),
    ("Errno::ENOSTR", "SystemCallError"),
    ("Errno::ENOSYS", "SystemCallError"),
    ("Errno::ENOTBLK", "SystemCallError"),
    ("Errno::ENOTCONN", "SystemCallError"),
    ("Errno::ENOTDIR", "SystemCallError"),
    ("Errno::ENOTEMPTY", "SystemCallError"),
    ("Errno::ENOTRECOVERABLE", "SystemCallError"),
    ("Errno::ENOTSOCK", "SystemCallError"),
    ("Errno::ENOTSUP", "SystemCallError"),
    ("Errno::ENOTTY", "SystemCallError"),
    ("Errno::ENXIO", "SystemCallError"),
    ("Errno::EOPNOTSUPP", "SystemCallError"),
    ("Errno::EOVERFLOW", "SystemCallError"),
    ("Errno::EOWNERDEAD", "SystemCallError"),
    ("Errno::EPERM", "SystemCallError"),
    ("Errno::EPFNOSUPPORT", "SystemCallError"),
    ("Errno::EPIPE", "SystemCallError"),
    ("Errno::EPROCLIM", "SystemCallError"),
    ("Errno::EPROCUNAVAIL", "SystemCallError"),
    ("Errno::EPROGMISMATCH", "SystemCallError"),
    ("Errno::EPROGUNAVAIL", "SystemCallError"),
    ("Errno::EPROTO", "SystemCallError"),
    ("Errno::EPROTONOSUPPORT", "SystemCallError"),
    ("Errno::EPROTOTYPE", "SystemCallError"),
    ("Errno::EPWROFF", "SystemCallError"),
    ("Errno::EQFULL", "SystemCallError"),
    ("Errno::ERANGE", "SystemCallError"),
    ("Errno::EREMOTE", "SystemCallError"),
    ("Errno::EROFS", "SystemCallError"),
    ("Errno::ERPCMISMATCH", "SystemCallError"),
    ("Errno::ESHLIBVERS", "SystemCallError"),
    ("Errno::ESHUTDOWN", "SystemCallError"),
    ("Errno::ESOCKTNOSUPPORT", "SystemCallError"),
    ("Errno::ESPIPE", "SystemCallError"),
    ("Errno::ESRCH", "SystemCallError"),
    ("Errno::ESTALE", "SystemCallError"),
    ("Errno::ETIME", "SystemCallError"),
    ("Errno::ETIMEDOUT", "SystemCallError"),
    ("Errno::ETOOMANYREFS", "SystemCallError"),
    ("Errno::ETXTBSY", "SystemCallError"),
    ("Errno::EUSERS", "SystemCallError"),
    ("Errno::EXDEV", "SystemCallError"),
    ("Errno::NOERROR", "SystemCallError"),
    ("ErrorHighlight::Spotter::NonAscii", "Exception"),
    ("FiberError", "StandardError"),
    ("FloatDomainError", "RangeError"),
    ("FrozenError", "RuntimeError"),
    ("Gem::CommandLineError", "Gem::Exception"),
    ("Gem::ConflictError", "Gem::LoadError"),
    ("Gem::DependencyError", "Gem::Exception"),
    ("Gem::DependencyRemovalException", "Gem::Exception"),
    ("Gem::DependencyResolutionError", "Gem::DependencyError"),
    ("Gem::DocumentError", "Gem::Exception"),
    ("Gem::EndOfYAMLException", "Gem::Exception"),
    ("Gem::Exception", "RuntimeError"),
    ("Gem::FilePermissionError", "Gem::Exception"),
    ("Gem::FormatException", "Gem::Exception"),
    ("Gem::GemNotFoundException", "Gem::Exception"),
    ("Gem::GemNotInHomeException", "Gem::Exception"),
    ("Gem::ImpossibleDependenciesError", "Gem::Exception"),
    ("Gem::InstallError", "Gem::Exception"),
    ("Gem::InvalidSpecificationException", "Gem::Exception"),
    ("Gem::LoadError", "LoadError"),
    ("Gem::MissingSpecError", "Gem::LoadError"),
    ("Gem::MissingSpecVersionError", "Gem::MissingSpecError"),
    ("Gem::OperationNotSupportedError", "Gem::Exception"),
    ("Gem::RemoteError", "Gem::Exception"),
    ("Gem::RemoteInstallationCancelled", "Gem::Exception"),
    ("Gem::RemoteInstallationSkipped", "Gem::Exception"),
    ("Gem::RemoteSourceException", "Gem::Exception"),
    ("Gem::Requirement::BadRequirementError", "ArgumentError"),
    ("Gem::RubyVersionMismatch", "Gem::Exception"),
    ("Gem::RuntimeRequirementNotMetError", "Gem::InstallError"),
    ("Gem::SpecificGemNotFoundException", "Gem::GemNotFoundException"),
    ("Gem::SystemExitException", "SystemExit"),
    ("Gem::UninstallError", "Gem::Exception"),
    ("Gem::UnknownCommandError", "Gem::Exception"),
    ("Gem::UnsatisfiableDependencyError", "Gem::DependencyError"),
    ("Gem::VerificationError", "Gem::Exception"),
    ("Gem::WebauthnVerificationError", "Gem::Exception"),
    ("IO::Buffer::AccessError", "RuntimeError"),
    ("IO::Buffer::AllocationError", "RuntimeError"),
    ("IO::Buffer::InvalidatedError", "RuntimeError"),
    ("IO::Buffer::LockedError", "RuntimeError"),
    ("IO::Buffer::MaskError", "ArgumentError"),
    ("IO::EAGAINWaitReadable", "Errno::EAGAIN"),
    ("IO::EAGAINWaitWritable", "Errno::EAGAIN"),
    ("IO::EINPROGRESSWaitReadable", "Errno::EINPROGRESS"),
    ("IO::EINPROGRESSWaitWritable", "Errno::EINPROGRESS"),
    ("IO::TimeoutError", "IOError"),
    ("IOError", "StandardError"),
    ("IndexError", "StandardError"),
    ("Interrupt", "SignalException"),
    ("KeyError", "IndexError"),
    ("LoadError", "ScriptError"),
    ("LocalJumpError", "StandardError"),
    ("Math::DomainError", "StandardError"),
    ("NameError", "StandardError"),
    ("NoMatchingPatternError", "StandardError"),
    ("NoMatchingPatternKeyError", "NoMatchingPatternError"),
    ("NoMemoryError", "Exception"),
    ("NoMethodError", "NameError"),
    ("NotImplementedError", "ScriptError"),
    ("Ractor::ClosedError", "StopIteration"),
    ("Ractor::Error", "RuntimeError"),
    ("Ractor::IsolationError", "Ractor::Error"),
    ("Ractor::MovedError", "Ractor::Error"),
    ("Ractor::RemoteError", "Ractor::Error"),
    ("Ractor::UnsafeError", "Ractor::Error"),
    ("RangeError", "StandardError"),
    ("Regexp::TimeoutError", "RegexpError"),
    ("RegexpError", "StandardError"),
    ("RuntimeError", "StandardError"),
    ("ScriptError", "Exception"),
    ("SecurityError", "Exception"),
    ("SignalException", "Exception"),
    ("StandardError", "Exception"),
    ("StopIteration", "IndexError"),
    ("SyntaxError", "ScriptError"),
    ("SystemCallError", "StandardError"),
    ("SystemExit", "Exception"),
    ("SystemStackError", "Exception"),
    ("ThreadError", "StandardError"),
    ("TypeError", "StandardError"),
    ("UncaughtThrowError", "ArgumentError"),
    ("ZeroDivisionError", "StandardError"),
    ("fatal", "Exception"),
];

/// `Kernel.const_get(exception.source)`, over [`EXCEPTION_HIERARCHY`] plus
/// the `Exception` root (which has no row of its own, being nobody's
/// child). Any other raw text -- custom classes, splats, bare calls,
/// literal arrays -- resolves to `None`, mirroring the `rescue NameError`
/// branch of `evaluate_exceptions`.
fn resolve(text: &[u8]) -> Option<&'static str> {
    let text = std::str::from_utf8(text).ok()?;
    if text == "Exception" {
        return Some("Exception");
    }
    EXCEPTION_HIERARCHY.iter().find(|(name, _)| *name == text).map(|&(name, _)| name)
}

/// `Exception`'s row is absent by construction (nobody's `superclass` here
/// points past it, since the table only has entries with a real
/// `superclass`); every other known name has exactly one.
fn superclass_of(name: &str) -> Option<&'static str> {
    if name == "Exception" {
        return None;
    }
    EXCEPTION_HIERARCHY.iter().find_map(|&(c, p)| if c == name { Some(p) } else { None })
}

/// Is `ancestor` a strict ancestor of `descendant` (`descendant < ancestor`
/// in Ruby)? Both names are assumed already resolved (present in the
/// table, or `Exception` itself); the walk is finite because
/// [`EXCEPTION_HIERARCHY`] only contains classes RuboCop's own generator
/// confirmed satisfy `c < Exception`, so every chain terminates there.
fn is_strict_ancestor(descendant: &str, ancestor: &str) -> bool {
    let mut current = descendant;
    while let Some(parent) = superclass_of(current) {
        if parent == ancestor {
            return true;
        }
        current = parent;
    }
    false
}

/// `Module#<=>` between two known exception classes: `Equal` when the same
/// class, `Less` when `a` descends from `b` (so `a < b`), `Greater` when
/// `b` descends from `a`, `None` when unrelated.
fn class_cmp(a: &str, b: &str) -> Option<Ordering> {
    if a == b {
        Some(Ordering::Equal)
    } else if is_strict_ancestor(a, b) {
        Some(Ordering::Less)
    } else if is_strict_ancestor(b, a) {
        Some(Ordering::Greater)
    } else {
        None
    }
}

/// `system_call_err?`: `error.ancestors[1] == SystemCallError`, i.e. `error`
/// is an exact immediate subclass of `SystemCallError` (every `Errno::*`
/// class, and nothing that descends from one of those, e.g.
/// `IO::EAGAINWaitReadable`).
fn is_system_call_err(name: &str) -> bool {
    superclass_of(name) == Some("SystemCallError")
}

/// `compare_exceptions`. See the module doc comment for why the
/// `system_call_err?` branch always returns `false`.
fn compare_exceptions(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => {
            if is_system_call_err(a) && is_system_call_err(b) {
                false
            } else {
                class_cmp(a, b).is_some()
            }
        }
        _ => false,
    }
}

/// One `rescue`'s resolved exception list (`evaluate_exceptions`'s return
/// value already flattened: RuboCop's whitequark model wraps a comma list
/// in an `array` node that `ResbodyNode#exceptions` unwraps, Prism's
/// `RescueNode::exceptions` is already the flat list).
type Group = Vec<Option<&'static str>>;

/// `group.include?(Exception)`.
fn includes_exception(group: &Group) -> bool {
    group.contains(&Some("Exception"))
}

/// `group.none?` (no block): true when every element is falsy, i.e. every
/// element is `None` (an unresolved exception; a resolved `Some` is always
/// truthy, being a real class reference).
fn is_none_group(group: &Group) -> bool {
    group.iter().all(Option::is_none)
}

/// `contains_multiple_levels_of_exceptions?`.
fn contains_multiple_levels(group: &Group) -> bool {
    if group.len() > 1 && includes_exception(group) {
        return true;
    }
    for i in 0..group.len() {
        for j in (i + 1)..group.len() {
            if compare_exceptions(group[i], group[j]) {
                return true;
            }
        }
    }
    false
}

/// One element's `<=>` inside `Array#<=>`: `nil.<=>` is `0` only against
/// another `nil`, else `nil`; two resolved classes compare via
/// [`class_cmp`] (plain ancestry, not `compare_exceptions`'s
/// `system_call_err?` special case -- `sorted?` calls `x <=> y` directly on
/// the arrays).
fn elem_cmp(a: Option<&str>, b: Option<&str>) -> Option<Ordering> {
    match (a, b) {
        (None, None) => Some(Ordering::Equal),
        (Some(a), Some(b)) => class_cmp(a, b),
        _ => None,
    }
}

/// `Array#<=>`: element-wise until a non-equal or `nil` comparison, else
/// compares lengths.
fn array_cmp(x: &Group, y: &Group) -> Option<Ordering> {
    for (a, b) in x.iter().zip(y) {
        match elem_cmp(*a, *b) {
            Some(Ordering::Equal) => {}
            other => return other,
        }
    }
    Some(x.len().cmp(&y.len()))
}

/// The body of `sorted?`'s `each_cons(2).all?` block, for one consecutive
/// pair -- reused as-is by `find_shadowing_rescue`, which calls `sorted?`
/// on a bare two-element slice (the same shape `each_cons(2)` yields here).
fn sorted_pair(x: &Group, y: &Group) -> bool {
    if includes_exception(x) {
        false
    } else if includes_exception(y) || is_none_group(x) || is_none_group(y) {
        true
    } else {
        !matches!(array_cmp(x, y), Some(Ordering::Greater))
    }
}

/// `sorted?`.
fn sorted(groups: &[Group]) -> bool {
    groups.windows(2).all(|w| sorted_pair(&w[0], &w[1]))
}

/// `find_shadowing_rescue`'s index into `groups`/the resbody chain, or
/// `None` if nothing is found (unreachable in practice: `on_rescue` only
/// calls this after confirming a shadowing rescue exists, same as
/// upstream, which would otherwise `add_offense(nil)`).
fn find_shadowing_index(groups: &[Group]) -> Option<usize> {
    if let Some(i) = groups.iter().position(contains_multiple_levels) {
        return Some(i);
    }
    (0..groups.len().saturating_sub(1)).find(|&i| !sorted_pair(&groups[i], &groups[i + 1]))
}

/// `evaluate_exceptions`: an empty `rescue` (no exceptions listed) is
/// treated as `rescue StandardError`; otherwise each exception expression's
/// raw source text is resolved via [`resolve`].
fn evaluate_exceptions(ctx: &Context<'_>, rescue_node: &RescueNode<'_>) -> Group {
    let exceptions = rescue_node.exceptions();
    if exceptions.is_empty() {
        vec![Some("StandardError")]
    } else {
        exceptions.iter().map(|node| resolve(ctx.text(node.span()))).collect()
    }
}

/// RuboCop's `offense_range`, recomputed from the clause's own children
/// instead of `RescueNode::location` (see the module doc comment for why).
fn shadowing_span(rescue_node: &RescueNode<'_>) -> Span {
    let start = rescue_node.keyword_loc().span().start;
    let end = if let Some(statements) = rescue_node.statements() {
        statements.location().span().end
    } else if let Some(reference) = rescue_node.reference() {
        reference.span().end
    } else if let Some(operator_loc) = rescue_node.operator_loc() {
        operator_loc.span().end
    } else if let Some(last_exception) = rescue_node.exceptions().last() {
        last_exception.span().end
    } else {
        rescue_node.keyword_loc().span().end
    };
    Span::new(start, end)
}

/// Checks for a rescued exception that gets shadowed by a less specific
/// exception being rescued before a more specific exception is rescued.
#[derive(Debug, Clone)]
pub struct ShadowedException;

impl Rule for ShadowedException {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ShadowedException",
        department: Department::Lint,
        summary: "Checks for a rescued exception that get shadowed by a less specific exception \
                  being rescued before a more specific exception is rescued.",
        explanation: "\
An exception is considered shadowed if it is rescued after its
ancestor is, or if it and its ancestor are both rescued in the
same `rescue` statement. In both cases, the more specific rescue is
unnecessary because it is covered by rescuing the less specific
exception. (ie. `rescue Exception, StandardError` has the same behavior
whether `StandardError` is included or not, because all `StandardError`s
are rescued by `rescue Exception`).

```ruby
# bad

begin
  something
rescue Exception
  handle_exception
rescue StandardError
  handle_standard_error
end

# bad
begin
  something
rescue Exception, StandardError
  handle_error
end

# good

begin
  something
rescue StandardError
  handle_standard_error
rescue Exception
  handle_exception
end

# good, however depending on runtime environment.
#
# This is a special case for system call errors.
# System dependent error code depends on runtime environment.
# For example, whether `Errno::EAGAIN` and `Errno::EWOULDBLOCK` are
# the same error code or different error code depends on environment.
# This good case is for `Errno::EAGAIN` and `Errno::EWOULDBLOCK` with
# the same error code.
begin
  something
rescue Errno::EAGAIN, Errno::EWOULDBLOCK
  handle_standard_error
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::BeginNode],
        config: &[],
        blind_spots: "\
Exception resolution is a static lookup against a table of every built-in
`c < Exception` class generated on one real Ruby process (see
`EXCEPTION_HIERARCHY`'s doc comment), not an actual `Kernel.const_get`:
custom exception classes defined elsewhere in the same codebase (or
reopened core classes) are always unresolved, exactly as they would be in
a RuboCop run where that file was never `require`d -- this only differs
from a full-project RuboCop run when the linted file itself defines and
immediately rescues its own custom hierarchy in a way that would already
be loaded by the time the cop's process resolves constants, which
RuboCop's own architecture (one process, `require`s happen via the
target application's boot, not the linter) makes exceedingly rare.
`Errno::*` pairs are never flagged for containing multiple levels
regardless of real platform error-code aliasing, matching every reachable
case of upstream's own `system_call_err?` special-casing (see the module
doc comment for the proof).",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let begin = node.as_begin_node().expect("kind matched");
        let Some(head) = begin.rescue_clause() else { return };

        let mut rescues = Vec::new();
        let mut current = Some(head);
        while let Some(r) = current {
            current = r.subsequent();
            rescues.push(r);
        }

        let groups: Vec<Group> = rescues.iter().map(|r| evaluate_exceptions(ctx, r)).collect();
        let rescues_multiple_levels = groups.iter().any(contains_multiple_levels);

        if !rescues_multiple_levels && sorted(&groups) {
            return;
        }

        if let Some(idx) = find_shadowing_index(&groups) {
            let span = shadowing_span(&rescues[idx]);
            ctx.report(&Self::META, span, MSG);
        }
    }
}
