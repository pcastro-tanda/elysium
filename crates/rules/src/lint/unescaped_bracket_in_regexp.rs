//! `Lint/UnescapedBracketInRegexp`, ported from RuboCop's
//! `lib/rubocop/cop/lint/unescaped_bracket_in_regexp.rb`.
//!
//! # No `regexp_parser` gem: a byte-level bracket-class scanner
//!
//! Upstream parses the regexp's pattern text with the `regexp_parser` gem
//! and walks its expression tree (`parsed_tree.each_expression`), tracking
//! whether each `Character::Set` (`[...]`) expression is empty so a `]`
//! immediately following `[`/`[^]` -- conventionally a literal member, not
//! the class terminator -- is skipped once rather than flagged or treated
//! as a close. There is no Rust port of that gem, so this rule instead
//! scans the raw (still-escaped) pattern bytes directly with an equivalent
//! single-pass state machine ([`scan_brackets`]): it tracks whether it is
//! inside an (unescaped, unterminated) `[...]` class and whether that class
//! has consumed any member yet. `\X` escape pairs are skipped two bytes at a
//! time (never re-examined as a bracket) and always count as a class member
//! when inside one; an unescaped `[` opens a class (consuming a following
//! `^` as negation, which does not itself count as a member); an unescaped
//! `]` is a literal class member (skipped, not an offense) only when it is
//! the class's first member, otherwise it closes an open class, otherwise
//! -- outside any class -- it is a genuine unescaped bracket and is
//! reported, unless it is the very first byte of the whole pattern (Ruby
//! does not warn there either, matching upstream's `(expr.ts + pos).zero?`
//! guard). This reproduces every fixture under
//! `fixtures/lint/unescaped_bracket_in_regexp/`, including the malformed-
//! pattern fixtures (`Regexp.new("+")`, `Regexp.new("{42}")`,
//! `Regexp.new("\xff")`, ...) that exist upstream to prove
//! `parse_regexp`'s rescued failure doesn't crash the cop: none of those
//! patterns contain a `]`, so this scanner silently finds nothing in them
//! too, without needing to detect or replicate the parse failure itself.
//!
//! # Two independent entry points, one shared scanner
//!
//! `on_regexp` (here, [`NodeKind::RegularExpressionNode`] and
//! [`NodeKind::InterpolatedRegularExpressionNode`]) scans every regexp
//! literal's own content directly. `on_send` (here, [`NodeKind::CallNode`])
//! separately matches `Regexp.new`/`Regexp.compile`/`::Regexp.new`/
//! `::Regexp.compile` calls whose first argument is a plain (non-
//! interpolated) string literal -- RuboCop's `regexp_constructor?` node
//! pattern, `(send (const {nil? cbase} :Regexp) {:new :compile} $str ...)`
//! -- and scans that string's raw source content the same way, skipping
//! entirely (RuboCop's `node.each_descendant(:dstr).any?` guard) if any
//! argument contains string interpolation anywhere in its subtree.
//! [`RegularExpressionNode::content_loc`]/[`StringNode::content_loc`]'s
//! *raw* source bytes (not `.unescaped()`, which would already have
//! resolved `\]` to `]` and erased the very distinction this cop looks
//! for) are scanned identically in both cases, and both offense ranges are
//! anchored the same way upstream computes them --
//! `node.loc.begin.end.adjust(...)`, the end of the opening delimiter --
//! which is exactly each node's own `content_loc` start.
//!
//! # `InterpolatedRegularExpressionNode`: scanned but untested
//!
//! No fixture exercises a regexp literal containing `#{...}` (only the
//! `Regexp.new`/`.compile` string-argument form has an interpolation
//! fixture, `containing_dstr_node_does_not_register_an_offense.rb`, and
//! that one is skipped entirely by the dstr guard before any scanning
//! happens). [`check_interpolated`] still threads the same [`BracketState`]
//! across each literal [`StringNode`] part in order (treating an
//! `EmbeddedStatementsNode` interpolation as one opaque, class-non-empty
//! byte of unknown content, matching the spirit of "we can't see inside
//! it") so a bracket class spanning an interpolation boundary like
//! `/[#{x}]/` is still tracked correctly, but this path has no fixture
//! coverage to confirm against upstream's actual `regexp_parser`-based
//! behaviour for such cases.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, NodeList};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Regular expression has `]` without escape.";

/// Checks for unescaped literal `]` in Regexp.
#[derive(Debug, Clone)]
pub struct UnescapedBracketInRegexp;

impl Rule for UnescapedBracketInRegexp {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UnescapedBracketInRegexp",
        department: Department::Lint,
        summary: "Checks for unescaped literal `]` in Regexp.",
        explanation: "\
Checks for unescaped literal `]` in Regexp.

```ruby
# bad
re = /[abc]]/

# good
re = /[abc]\\]/
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::RegularExpressionNode,
            NodeKind::InterpolatedRegularExpressionNode,
            NodeKind::CallNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::RegularExpressionNode { .. } => {
                let re = node.as_regular_expression_node().expect("kind matched");
                check_plain(ctx, re.content_loc().span());
            }
            Node::InterpolatedRegularExpressionNode { .. } => {
                let re = node.as_interpolated_regular_expression_node().expect("kind matched");
                check_interpolated(ctx, &re.parts());
            }
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("kind matched");
                check_constructor_call(ctx, node, &call);
            }
            _ => {}
        }
    }
}

/// Tracks the stack of currently-open (unescaped) bracket classes, each
/// recording whether it has consumed a member yet -- see
/// [`scan_brackets`]'s doc for why this must be a stack, not a flat flag.
#[derive(Default, Clone)]
struct BracketState {
    stack: Vec<bool>,
}

/// Standard Ruby/Oniguruma POSIX bracket-expression class names -- the
/// `regexp_parser` gem's `Regexp::Syntax::Token::PosixClass::All`
/// (`Standard + Extensions`).
const POSIX_CLASS_NAMES: &[&str] = &[
    "alnum", "alpha", "blank", "cntrl", "digit", "graph", "lower", "print", "punct", "space",
    "upper", "xdigit", "ascii", "word",
];

/// If `content[i..]` opens a POSIX bracket-expression (`[:name:]` or the
/// negated `[:^name:]`) -- only meaningful while already inside an open
/// `[...]` class, exactly like `regexp_parser`'s scanner, which only
/// recognises this grammar in its `in_set?` state -- returns the exclusive
/// end index just past the closing `:]` together with whether `name` is a
/// recognized POSIX class name. Returns `None` if `content[i..]` doesn't
/// have this shape at all, so the `[` there is just an ordinary class
/// member byte.
fn posix_class_at(content: &[u8], i: usize) -> Option<(usize, bool)> {
    if content.get(i) != Some(&b'[') || content.get(i + 1) != Some(&b':') {
        return None;
    }
    let mut j = i + 2;
    if content.get(j) == Some(&b'^') {
        j += 1;
    }
    let name_start = j;
    while content.get(j).is_some_and(u8::is_ascii_lowercase) {
        j += 1;
    }
    if j == name_start || content.get(j) != Some(&b':') || content.get(j + 1) != Some(&b']') {
        return None;
    }
    let name = std::str::from_utf8(&content[name_start..j]).unwrap_or("");
    Some((j + 2, POSIX_CLASS_NAMES.contains(&name)))
}

/// Scans `content` (raw, still-escaped source bytes) for unescaped `]`,
/// pushing each genuine offense's absolute offset onto `offsets` rather
/// than reporting it directly: an unrecognized POSIX class name anywhere
/// in the content means `regexp_parser` itself fails to parse the whole
/// regexp (RuboCop's `parse_regexp` rescues `Regexp::Parser::Error` and
/// returns `nil`), so upstream reports *no* offenses at all for that node
/// -- callers must discard `offsets` entirely when this returns `false`.
/// `base` is `content`'s own start offset, for absolute offense spans.
/// `at_very_start` is true only when `content` begins at byte 0 of the
/// whole pattern (so index 0 there is RuboCop's "very first char of the
/// regexp" exemption); it is cleared after the first byte scanned,
/// including across call boundaries for [`check_interpolated`]'s
/// multi-part threading.
///
/// `state` is a stack of per-open-class `class_empty` flags rather than a
/// single flag: Oniguruma (and so `regexp_parser`) allows a `[...]` to
/// nest another true `[...]` inside it directly (no escaping needed) for
/// set union/intersection, e.g. `[^[[:print:]]\t\n]`'s outer negated class
/// containing a nested `[[:print:]]` union member -- each nesting level
/// gets its own independent "has this class consumed a member yet" state,
/// and a `]` always closes only the innermost open level.
fn scan_brackets(
    content: &[u8],
    base: u32,
    state: &mut BracketState,
    at_very_start: &mut bool,
    offsets: &mut Vec<u32>,
) -> bool {
    let mut i = 0usize;
    while i < content.len() {
        match content[i] {
            b'\\' => {
                if let Some(class_empty) = state.stack.last_mut() {
                    *class_empty = false;
                }
                i += 2;
            }
            b'[' if state.stack.is_empty() => {
                state.stack.push(true);
                i += 1;
                if content.get(i) == Some(&b'^') {
                    i += 1;
                }
            }
            b'[' => match posix_class_at(content, i) {
                Some((end, true)) => {
                    if let Some(class_empty) = state.stack.last_mut() {
                        *class_empty = false;
                    }
                    i = end;
                }
                Some((_, false)) => return false,
                None => {
                    // A genuinely nested character class (Oniguruma set
                    // union/intersection), not a POSIX bracket-expression.
                    if let Some(class_empty) = state.stack.last_mut() {
                        *class_empty = false;
                    }
                    state.stack.push(true);
                    i += 1;
                    if content.get(i) == Some(&b'^') {
                        i += 1;
                    }
                }
            },
            b']' => {
                if *at_very_start && i == 0 {
                    // Ruby does not warn on an unescaped `]` as the very
                    // first character of the whole pattern.
                } else if let Some(class_empty) = state.stack.last_mut() {
                    if *class_empty {
                        // The class's first member, conventionally literal.
                        *class_empty = false;
                    } else {
                        // Closes only the innermost open class.
                        state.stack.pop();
                    }
                } else {
                    offsets.push(base + u32::try_from(i).expect("offset exceeds u32"));
                }
                i += 1;
            }
            _ => {
                if let Some(class_empty) = state.stack.last_mut() {
                    *class_empty = false;
                }
                i += 1;
            }
        }
        *at_very_start = false;
    }
    true
}

/// A plain (non-interpolated) regexp body or string-argument content:
/// a single contiguous scan starting fresh at this node's own start.
fn check_plain(ctx: &mut Context<'_>, content_span: Span) {
    let content = ctx.text(content_span).to_vec();
    let mut state = BracketState::default();
    let mut at_very_start = true;
    let mut offsets = Vec::new();
    if scan_brackets(&content, content_span.start, &mut state, &mut at_very_start, &mut offsets) {
        for offset in offsets {
            report(ctx, offset);
        }
    }
}

/// An interpolated regexp literal's parts, scanned in source order with
/// bracket-class state threaded across them. See the module doc's note on
/// why this path has no fixture coverage.
fn check_interpolated(ctx: &mut Context<'_>, parts: &NodeList<'_>) {
    let mut state = BracketState::default();
    let mut at_very_start = true;
    let mut offsets = Vec::new();
    let mut valid = true;
    for part in parts {
        if !valid {
            break;
        }
        if let Some(s) = part.as_string_node() {
            let content_span = s.content_loc().span();
            let content = ctx.text(content_span).to_vec();
            valid = scan_brackets(
                &content,
                content_span.start,
                &mut state,
                &mut at_very_start,
                &mut offsets,
            );
        } else {
            // An embedded `#{...}` statement: opaque, unknown content --
            // counts as a class member (we can't see inside it) but is
            // never itself a bracket.
            if let Some(class_empty) = state.stack.last_mut() {
                *class_empty = false;
            }
            at_very_start = false;
        }
    }
    if valid {
        for offset in offsets {
            report(ctx, offset);
        }
    }
}

/// RuboCop's `regexp_constructor?` matcher plus the `on_send` guards:
/// `Regexp.new`/`Regexp.compile`/`::Regexp.new`/`::Regexp.compile` with a
/// plain string literal first argument and no interpolation anywhere in
/// the call's own subtree.
fn check_constructor_call(ctx: &mut Context<'_>, node: &Node<'_>, call: &CallNode<'_>) {
    let name = call.name().as_slice();
    if name != b"new" && name != b"compile" {
        return;
    }
    if has_dstr_descendant(node) {
        return;
    }
    let Some(receiver) = call.receiver() else { return };
    if !is_regexp_const(&receiver) {
        return;
    }
    let Some(arguments) = call.arguments() else { return };
    let Some(first) = arguments.arguments().iter().next() else { return };
    let Some(string) = first.as_string_node() else { return };
    check_plain(ctx, string.content_loc().span());
}

/// RuboCop's `node.each_descendant(:dstr).any?`.
fn has_dstr_descendant(node: &Node<'_>) -> bool {
    let mut found = false;
    each_descendant(node, &mut |n| {
        if n.kind() == NodeKind::InterpolatedStringNode {
            found = true;
        }
    });
    found
}

/// RuboCop's `(const {nil? cbase} :Regexp)`: a bare `Regexp` constant, or
/// `::Regexp` (a `ConstantPathNode` with no parent, i.e. rooted at `::`).
fn is_regexp_const(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::ConstantReadNode => {
            node.as_constant_read_node().expect("kind matched").name().as_slice() == b"Regexp"
        }
        NodeKind::ConstantPathNode => {
            let path = node.as_constant_path_node().expect("kind matched");
            path.parent().is_none() && path.name().is_some_and(|name| name.as_slice() == b"Regexp")
        }
        _ => false,
    }
}

/// RuboCop's `add_offense(location) { |corrector| corrector.replace(location, '\]') }`.
fn report(ctx: &mut Context<'_>, bracket_offset: u32) {
    let span = Span::new(bracket_offset, bracket_offset + 1);
    let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::replace(span, *b"\\]")] };
    ctx.report_with_fix(&UnescapedBracketInRegexp::META, span, MSG, fix);
}
