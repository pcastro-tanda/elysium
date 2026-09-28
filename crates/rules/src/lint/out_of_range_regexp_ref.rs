//! `Lint/OutOfRangeRegexpRef`, ported from RuboCop's
//! `lib/rubocop/cop/lint/out_of_range_regexp_ref.rb`.
//!
//! # Approach
//!
//! Upstream tracks a single `@valid_ref` (the number of captures the most
//! recently seen `Regexp` literal offers, or `nil` once that count becomes
//! unknown) across four callbacks and reports `$N` (`on_nth_ref`) whenever it
//! exceeds that count. This port keeps the same single piece of state and the
//! same four hooks, mapped onto Prism nodes:
//!
//! - `after_send`/`after_csend` (a `send`/`csend` restricted to the receiver-
//!   or argument-regexp method names) becomes `enter`+`leave` hooks for
//!   `CallNode` (see [`OutOfRangeRegexpRef::pending`]'s doc for why this
//!   needs both, rather than a plain post-order `leave`).
//! - `on_when`/`on_in_pattern` become `enter` hooks for `WhenNode`/`InNode`
//!   (pre-order, since they must apply before their branch's body is
//!   visited).
//! - `on_nth_ref` becomes an `enter` hook for `NumberedReferenceReadNode`
//!   (Prism's `$1`..`$9`; whitequark's `nth_ref`).
//! - `on_match_with_lvasgn` (whitequark's special node for `/(?<x>...)/ =~
//!   str`, needed there only because a named-capture `=~` is *not* a `send`
//!   node in that AST at all) needs no separate handling here: Prism's
//!   equivalent, `MatchWriteNode`, always wraps a real `CallNode` in its
//!   `call` field, which the engine's generic tree walk still visits (and
//!   dispatches to this rule's `CallNode` handling) regardless of the
//!   `MatchWriteNode` wrapper around it.
//!
//! `check_regexp`'s capture count comes from a small private scanner (copied
//! from [`crate::lint::mixed_regexp_capture_types`], counting instead of just
//! detecting) instead of building `regexp_parser`'s full tree; see that
//! module's doc for why this is expected to agree with upstream on every
//! fixture.
//!
//! `regexp_type?` (whitequark's single `:regexp` node type, covering both
//! literal and interpolated regexps alike) is Prism's *pair* of node kinds,
//! `RegularExpressionNode` and `InterpolatedRegularExpressionNode`; this port
//! checks for either wherever upstream checks `regexp_type?`, and mirrors
//! `node.interpolation?` (true only for the latter) inside `check_regexp`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::{CallNode, InNode, WhenNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// RuboCop's `REGEXP_RECEIVER_METHODS`.
const REGEXP_RECEIVER_METHODS: &[&[u8]] = &[b"=~", b"===", b"match"];
/// RuboCop's `REGEXP_ARGUMENT_METHODS`.
const REGEXP_ARGUMENT_METHODS: &[&[u8]] = &[
    b"=~",
    b"match",
    b"grep",
    b"gsub",
    b"gsub!",
    b"sub",
    b"sub!",
    b"[]",
    b"slice",
    b"slice!",
    b"index",
    b"rindex",
    b"scan",
    b"partition",
    b"rpartition",
    b"start_with?",
    b"end_with?",
];

/// The pending `after_send` effect of a `CallNode` currently being visited,
/// keyed by nesting depth (see [`OutOfRangeRegexpRef::pending`]).
#[derive(Debug, Clone, Copy)]
enum Pending {
    /// This call's method isn't in the receiver/argument method sets, so it
    /// has no effect on `@valid_ref` (nothing to apply, ever).
    NotEligible,
    /// This call's effect on `@valid_ref`, not yet applied.
    Eligible(Option<i64>),
}

/// Checks for out of range reference for Regexp because it always returns nil.
#[derive(Debug, Clone)]
pub struct OutOfRangeRegexpRef {
    /// RuboCop's `@valid_ref`: the capture count of the most recently seen
    /// regexp (`Some`), or `None` once that count becomes unknown.
    valid_ref: Option<i64>,
    /// One entry per `CallNode` currently open on the path from the root,
    /// holding that call's not-yet-applied `after_send` effect (if any).
    ///
    /// Whitequark's `block` node *wraps* its `send` child, so `after_send`
    /// (post-order on the `send` node alone) fires strictly before the
    /// block's own body is visited. Prism's `CallNode` instead holds its
    /// block as a direct child field, so the generic tree walk would visit
    /// the block *before* a plain `leave`-hook `after_send` gets a chance to
    /// run. This stack defers each eligible call's effect until either its
    /// `BlockNode` child is entered (applied there, right before the
    /// block's body -- matching whitequark's timing) or, if it has none,
    /// its own `leave` (applied there instead, once receiver/arguments are
    /// fully done -- also matching whitequark, since there is no block body
    /// to have run first).
    pending: Vec<Pending>,
}

impl Rule for OutOfRangeRegexpRef {
    const META: RuleMeta = RuleMeta {
        name: "Lint/OutOfRangeRegexpRef",
        department: Department::Lint,
        summary: "Checks for out of range reference for Regexp because it always returns nil.",
        explanation: "\
Looks for references of `Regexp` captures that are out of range
and thus always returns nil.

## Safety

This cop is unsafe because it is naive in how it determines what
references are available based on the last encountered regexp, but
it cannot handle some cases, such as conditional regexp matches, which
leads to false positives, such as:

```ruby
foo ? /(c)(b)/ =~ str : /(b)/ =~ str
do_something if $2
# $2 is defined for the first condition but not the second, however
# the cop will mark this as an offense.
```

This might be a good indication of code that should be refactored,
however.

```ruby
/(foo)bar/ =~ 'foobar'

# bad - always returns nil

puts $2 # => nil

# good

puts $1 # => foo
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::BlockNode,
            NodeKind::WhenNode,
            NodeKind::InNode,
            NodeKind::NumberedReferenceReadNode,
        ],
        config: &[],
        blind_spots: "\
RuboCop's `regexp_parser`-based capture scan silently yields no captures at \
all for a pattern it fails to parse; this port's simpler scanner never fails \
to parse and always finds whatever named/numbered captures its grammar \
recognizes. No known fixture distinguishes the two.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { valid_ref: None, pending: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        // RuboCop's `on_new_investigation`.
        self.valid_ref = Some(0);
        self.pending.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                self.pending.push(Self::after_send_effect(&call, ctx));
            }
            NodeKind::BlockNode => {
                // The block's owning `CallNode` is always the innermost
                // still-open call (see `pending`'s doc); apply its effect
                // now, before descending into the block's own body, and
                // mark it applied so `leave` doesn't reapply (and
                // potentially clobber a further update the block's body
                // itself makes) once we get there.
                if let Some(slot) = self.pending.last_mut() {
                    if let Pending::Eligible(new_valid_ref) = *slot {
                        self.valid_ref = new_valid_ref;
                        *slot = Pending::NotEligible;
                    }
                }
            }
            NodeKind::WhenNode => {
                let when = node.as_when_node().expect("kind matched");
                self.on_when(&when, ctx);
            }
            NodeKind::InNode => {
                let in_node = node.as_in_node().expect("kind matched");
                self.on_in_pattern(&in_node, ctx);
            }
            NodeKind::NumberedReferenceReadNode => self.on_nth_ref(node, ctx),
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        let NodeKind::CallNode = node.kind() else { return };
        if let Pending::Eligible(new_valid_ref) = self.pending.pop().expect("pushed on enter") {
            self.valid_ref = new_valid_ref;
        }
    }
}

impl OutOfRangeRegexpRef {
    /// RuboCop's `after_send`/`after_csend`: the `@valid_ref` this call
    /// would set (if its method is restricted-on), computed purely from the
    /// call's own fields (no traversal dependency) so it can run as soon as
    /// the call is entered, independent of when its effect is applied.
    fn after_send_effect(node: &CallNode<'_>, ctx: &Context<'_>) -> Pending {
        let name = node.name();
        let name = name.as_slice();
        if !REGEXP_RECEIVER_METHODS.contains(&name) && !REGEXP_ARGUMENT_METHODS.contains(&name) {
            return Pending::NotEligible;
        }

        if REGEXP_ARGUMENT_METHODS.contains(&name) {
            if let Some(first) = node.arguments().and_then(|args| args.arguments().first()) {
                if is_regexp_type(&first) {
                    return Pending::Eligible(check_regexp(ctx, &first));
                }
            }
        }
        if let Some(receiver) = node.receiver() {
            if is_regexp_type(&receiver) {
                return Pending::Eligible(check_regexp(ctx, &receiver));
            }
        }
        Pending::Eligible(None)
    }

    /// RuboCop's `on_when`.
    fn on_when(&mut self, node: &WhenNode<'_>, ctx: &Context<'_>) {
        let mut counts: Vec<i64> = Vec::new();
        for condition in &node.conditions() {
            if is_regexp_type(&condition) {
                if let Some(count) = check_regexp(ctx, &condition) {
                    counts.push(count);
                }
            }
        }
        self.valid_ref = counts.into_iter().max();
    }

    /// RuboCop's `on_in_pattern`.
    fn on_in_pattern(&mut self, node: &InNode<'_>, ctx: &Context<'_>) {
        let pattern = node.pattern();
        let mut counts: Vec<i64> = Vec::new();
        if is_regexp_type(&pattern) {
            if let Some(count) = check_regexp(ctx, &pattern) {
                counts.push(count);
            }
        } else {
            ruby_ast::each_descendant(&pattern, &mut |descendant| {
                if is_regexp_type(descendant) {
                    if let Some(count) = check_regexp(ctx, descendant) {
                        counts.push(count);
                    }
                }
            });
        }
        self.valid_ref = counts.into_iter().max();
    }

    /// RuboCop's `on_nth_ref`.
    fn on_nth_ref(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let n = node.as_numbered_reference_read_node().expect("kind matched");
        let backref = i64::from(n.number());
        let Some(valid_ref) = self.valid_ref else { return };
        if backref <= valid_ref {
            return;
        }

        let count = if valid_ref == 0 { "no".to_string() } else { valid_ref.to_string() };
        let group = if valid_ref == 1 { "group" } else { "groups" };
        let message =
            format!("${backref} is out of range ({count} regexp capture {group} detected).");
        ctx.report(&Self::META, node.span(), message);
    }
}

/// `regexp_type?`: whitequark's single `:regexp` node type covers both of
/// Prism's plain and interpolated regexp literal kinds.
fn is_regexp_type(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::RegularExpressionNode | NodeKind::InterpolatedRegularExpressionNode
    )
}

/// RuboCop's `check_regexp`: returns the regexp literal's capture count
/// (named captures if any exist, else numbered captures), or `None` for an
/// interpolated regexp (`node.interpolation?`), mirroring `check_regexp`'s
/// early `return` -- which leaves `@valid_ref` untouched by the caller.
fn check_regexp(ctx: &Context<'_>, node: &Node<'_>) -> Option<i64> {
    let n = node.as_regular_expression_node()?;
    let buf = ctx.text(n.content_loc().span());
    let (named, numbered) = count_captures(buf);
    Some(if named > 0 { named } else { numbered })
}

/// How an unescaped, non-class `(` at some position classifies, and how
/// many bytes of the group's opening syntax to skip past before resuming
/// the normal scan. Copied from [`crate::lint::mixed_regexp_capture_types`].
enum GroupKind {
    Named,
    Numbered,
    NonCapturing,
}

/// Classifies the group opened by `buf[pos] == '('`. Returns its kind and
/// the number of bytes (starting at `pos`) making up its opening syntax.
/// Copied from [`crate::lint::mixed_regexp_capture_types`].
fn classify_group(buf: &[u8], pos: usize, len: usize) -> (GroupKind, usize) {
    if pos + 1 >= len || buf[pos + 1] != b'?' {
        return (GroupKind::Numbered, 1);
    }
    if pos + 2 >= len {
        return (GroupKind::NonCapturing, 2);
    }
    match buf[pos + 2] {
        b':' | b'=' | b'!' | b'>' => (GroupKind::NonCapturing, 3),
        b'#' => {
            let mut i = pos + 3;
            while i < len && buf[i] != b')' {
                i += 1;
            }
            let end = if i < len { i + 1 } else { i };
            (GroupKind::NonCapturing, end - pos)
        }
        b'<' => {
            if pos + 3 < len && matches!(buf[pos + 3], b'=' | b'!') {
                (GroupKind::NonCapturing, 4)
            } else {
                (GroupKind::Named, 3)
            }
        }
        b'\'' => (GroupKind::Named, 3),
        _ => (GroupKind::NonCapturing, 2),
    }
}

/// Scans `buf` (a regexp literal's content bytes) for top-level named and
/// numbered capture groups, tracking character-class depth so that a `(`
/// inside `[...]` is never mistaken for a group. Returns the count of named
/// captures and the count of numbered captures. Adapted from
/// [`crate::lint::mixed_regexp_capture_types::scan_captures`] to count
/// instead of merely detect.
fn count_captures(buf: &[u8]) -> (i64, i64) {
    let len = buf.len();
    let mut pos = 0usize;
    let mut depth: u32 = 0;
    let mut named: i64 = 0;
    let mut numbered: i64 = 0;
    while pos < len {
        let c = buf[pos];
        if c == b'\\' {
            pos += 1;
            if pos >= len {
                break;
            }
            pos += utf8_len(buf, pos);
            continue;
        }
        if c == b'[' {
            depth += 1;
            pos += 1;
            continue;
        }
        if c == b']' && depth > 0 {
            depth -= 1;
            pos += 1;
            continue;
        }
        if depth == 0 && c == b'(' {
            let (kind, advance) = classify_group(buf, pos, len);
            match kind {
                GroupKind::Named => named += 1,
                GroupKind::Numbered => numbered += 1,
                GroupKind::NonCapturing => {}
            }
            pos += advance.max(1);
            continue;
        }
        pos += utf8_len(buf, pos);
    }
    (named, numbered)
}

/// The byte length of the UTF-8 sequence starting at `buf[pos]`, clamped to
/// the buffer's remaining length; `1` past the end or for a lone
/// continuation/invalid leading byte. Copied from
/// [`crate::lint::mixed_regexp_capture_types`].
fn utf8_len(buf: &[u8], pos: usize) -> usize {
    if pos >= buf.len() {
        return 1;
    }
    let b = buf[pos];
    let want = if b & 0x80 == 0 {
        1
    } else if b & 0xE0 == 0xC0 {
        2
    } else if b & 0xF0 == 0xE0 {
        3
    } else if b & 0xF8 == 0xF0 {
        4
    } else {
        1
    };
    want.min(buf.len() - pos)
}
