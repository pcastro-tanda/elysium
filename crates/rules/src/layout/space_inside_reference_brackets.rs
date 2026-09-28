//! `Layout/SpaceInsideReferenceBrackets`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_inside_reference_brackets.rb` plus the
//! `SurroundingSpace` mixin (`lib/rubocop/cop/mixin/surrounding_space.rb`)
//! and `SpaceCorrector` (`lib/rubocop/cop/correctors/space_corrector.rb`) it
//! uses.
//!
//! RuboCop's cop reasons over a *token stream* to find "its own" `[`/`]`
//! tokens (`left_ref_bracket`/`closing_bracket`), because a `parser`-gem
//! `send` node for `a[:key]["foo"]` shares the same token range as its
//! receiver, and the token search must walk past the receiver's own
//! brackets to land on this call's. Prism instead gives every `[]`/`[]=`
//! `CallNode` its own `opening_loc`/`closing_loc` fields directly (as with
//! `foo[bar]`'s parenthesis-shaped `opening_loc` elsewhere in this crate),
//! so that whole token-walking apparatus has no Prism equivalent to port:
//! [`brackets`] just reads the node's own locations.
//!
//! RuboCop's `on_send` only ever fires for method calls (never a `def
//! Vector.[](*array)` method definition, which is a distinct node type),
//! and is never aliased to `on_csend`, so a safe-navigation `[]` call
//! (`a&.[](1)`) is silently skipped -- reproduced here by rejecting
//! `is_safe_navigation`. Explicit method-call syntax (`subject.[](0)`) has
//! an `opening_loc` of `(`, not `[`, and is excluded by the same
//! `opening.as_slice() == b"["` filter `SpaceInsideArrayLiteralBrackets`
//! uses to exclude `%w[]`.
//!
//! Because this cop's own `on_send` calls `no_space_offenses`/
//! `space_offenses` with no `start_ok:`/`end_ok:` arguments (both default
//! `false` in the mixin) and unconditionally returns before them whenever
//! `node.multiline?`, the `next_to_comment?`/`next_to_newline?`/
//! `end_has_own_line?` special-casing that `SpaceInsideArrayLiteralBrackets`
//! needs for its `space`/`compact` styles never applies here: by the time
//! the style checks run, the whole call is already known to fit on one
//! line, so `start_ok`/`end_ok` are always `false` and are dropped from the
//! signatures below. `EnforcedStyleForEmptyBrackets` is still checked
//! *before* that multiline return, exactly like the array cop, so an empty
//! bracket pair with an interior newline (`a[\n]`) is still corrected.
use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::Node;
use ruby_ast::{LocationExt, NodeExt, NodeKind};
use ruby_source::{is_ruby_whitespace, Span};

/// RuboCop's `MSG`, formatted with `SPACE_COMMAND`.
const MSG_USE: &str = "Use space inside reference brackets.";
/// RuboCop's `MSG`, formatted with `NO_SPACE_COMMAND`.
const MSG_NO_USE: &str = "Do not use space inside reference brackets.";
/// RuboCop's `EMPTY_MSG`, formatted with `'Use one'`.
const MSG_EMPTY_USE_ONE: &str = "Use one space inside empty reference brackets.";
/// RuboCop's `EMPTY_MSG`, formatted with `NO_SPACE_COMMAND`.
const MSG_EMPTY_NO_USE: &str = "Do not use space inside empty reference brackets.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Space,
    NoSpace,
}

/// RuboCop's `EnforcedStyleForEmptyBrackets`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmptyStyle {
    Space,
    NoSpace,
}

/// `SurroundingSpace::SINGLE_SPACE_REGEXP`: plain space or tab only, never a
/// newline (which can only ever occur here inside empty brackets, handled
/// separately by [`check_empty`]).
const fn is_space_or_tab(b: u8) -> bool {
    matches!(b, b' ' | b'\t')
}

/// `extra_space?(token, :left)`-alike: a plain space/tab right after `pos`.
fn extra_space_after(bytes: &[u8], pos: u32) -> bool {
    bytes.get(pos as usize).is_some_and(|&b| is_space_or_tab(b))
}

/// `extra_space?(token, :right)`-alike: a plain space/tab right before `pos`.
fn extra_space_before(bytes: &[u8], pos: u32) -> bool {
    pos > 0 && is_space_or_tab(bytes[pos as usize - 1])
}

/// `SurroundingSpace#reposition(src, pos, +1, include_newlines: false)`:
/// extends `pos` forward over a run of plain spaces/tabs.
fn reposition_forward(bytes: &[u8], mut pos: u32) -> u32 {
    while bytes.get(pos as usize).is_some_and(|&b| is_space_or_tab(b)) {
        pos += 1;
    }
    pos
}

/// `SurroundingSpace#reposition(src, pos, -1, include_newlines: false)`:
/// extends `pos` backward over a run of plain spaces/tabs.
fn reposition_backward(bytes: &[u8], mut pos: u32) -> u32 {
    while pos > 0 && is_space_or_tab(bytes[pos as usize - 1]) {
        pos -= 1;
    }
    pos
}

/// Resolves the `[`/`]` bracket spans of a `[]`/`[]=` call, restricted (like
/// RuboCop's token-stream search, which only ever matches a literal `[`) to
/// a literal single-byte `[` opening -- excluding both explicit
/// method-call syntax (`subject.[](0)`, whose "opening" is a `(`) and
/// safe-navigated calls (`a&.[](1)`, never dispatched to by RuboCop's
/// `on_send` in the first place).
fn brackets(node: &Node<'_>) -> Option<(Span, Span)> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() || !matches!(call.name().as_slice(), b"[]" | b"[]=") {
        return None;
    }
    let opening = call.opening_loc()?;
    if opening.as_slice() != b"[" {
        return None;
    }
    let closing = call.closing_loc()?;
    Some((opening.span(), closing.span()))
}

/// Reports one offense with its accompanying fix, mirroring `space_offense`/
/// `empty_offense`: `fix_span` replaced by `replacement` (empty means
/// delete, a zero-width `fix_span` means insert).
fn emit(
    ctx: &mut Context<'_>,
    span: Span,
    message: &'static str,
    fix_span: Span,
    replacement: &'static [u8],
) {
    let fix = Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::replace(fix_span, replacement)],
    };
    ctx.report_with_fix(&SpaceInsideReferenceBrackets::META, span, message, fix);
}

/// `no_space_offenses`, left side.
fn no_space_left(open: Span, bytes: &[u8], ctx: &mut Context<'_>) {
    if !extra_space_after(bytes, open.end) {
        return;
    }
    let span = Span::new(open.end, reposition_forward(bytes, open.end));
    emit(ctx, span, MSG_NO_USE, span, b"");
}

/// `no_space_offenses`, right side.
fn no_space_right(close: Span, bytes: &[u8], ctx: &mut Context<'_>) {
    if !extra_space_before(bytes, close.start) {
        return;
    }
    let span = Span::new(reposition_backward(bytes, close.start), close.start);
    emit(ctx, span, MSG_NO_USE, span, b"");
}

/// `space_offenses`, left side.
fn space_left(open: Span, bytes: &[u8], ctx: &mut Context<'_>) {
    if extra_space_after(bytes, open.end) {
        return;
    }
    emit(ctx, open, MSG_USE, Span::new(open.end, open.end), b" ");
}

/// `space_offenses`, right side.
fn space_right(close: Span, bytes: &[u8], ctx: &mut Context<'_>) {
    if extra_space_before(bytes, close.start) {
        return;
    }
    emit(ctx, close, MSG_USE, Span::new(close.start, close.start), b" ");
}

/// `empty_offenses`/`SpaceCorrector.empty_corrections`: brackets with
/// nothing but whitespace between them.
fn check_empty(
    open: Span,
    close: Span,
    empty_style: EmptyStyle,
    bytes: &[u8],
    ctx: &mut Context<'_>,
) {
    let full = Span::new(open.start, close.end);
    let between = Span::new(open.end, close.start);
    match empty_style {
        EmptyStyle::Space => {
            let exactly_one_space = between.len() == 1 && bytes[open.end as usize] == b' ';
            if !exactly_one_space {
                emit(ctx, full, MSG_EMPTY_USE_ONE, between, b" ");
            }
        }
        EmptyStyle::NoSpace => {
            if !between.is_empty() {
                emit(ctx, full, MSG_EMPTY_NO_USE, between, b"");
            }
        }
    }
}

/// `on_send`: dispatches to the empty-brackets check (unconditional) or,
/// for single-line calls only, the style-specific left/right checks.
fn check(
    node: &Node<'_>,
    open: Span,
    close: Span,
    style: Style,
    empty_style: EmptyStyle,
    ctx: &mut Context<'_>,
) {
    let bytes = ctx.source().bytes();
    let between = Span::new(open.end, close.start);
    if bytes[between.range()].iter().all(|&b| is_ruby_whitespace(b)) {
        check_empty(open, close, empty_style, bytes, ctx);
        return;
    }

    if !ctx.is_single_line(node.span()) {
        return;
    }

    match style {
        Style::NoSpace => {
            no_space_left(open, bytes, ctx);
            no_space_right(close, bytes, ctx);
        }
        Style::Space => {
            space_left(open, bytes, ctx);
            space_right(close, bytes, ctx);
        }
    }
}

/// Checks that reference brackets have or don't have surrounding space
/// depending on configuration.
#[derive(Debug, Clone)]
pub struct SpaceInsideReferenceBrackets {
    style: Style,
    empty_style: EmptyStyle,
}

impl Rule for SpaceInsideReferenceBrackets {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceInsideReferenceBrackets",
        department: Department::Layout,
        summary: "Checks the spacing inside referential brackets.",
        explanation: "\
Checks that reference brackets have or don't have surrounding space
depending on configuration.

```ruby
# EnforcedStyle: no_space (default)
# The `no_space` style enforces that reference brackets have
# no surrounding space.

# bad
hash[ :key ]
array[ index ]

# good
hash[:key]
array[index]
```

```ruby
# EnforcedStyle: space
# The `space` style enforces that reference brackets have
# surrounding space.

# bad
hash[:key]
array[index]

# good
hash[ :key ]
array[ index ]
```

```ruby
# EnforcedStyleForEmptyBrackets: no_space (default)
# The `no_space` EnforcedStyleForEmptyBrackets style enforces that
# empty reference brackets do not contain spaces.

# bad
foo[ ]
foo[     ]
foo[
]

# good
foo[]
```

```ruby
# EnforcedStyleForEmptyBrackets: space
# The `space` EnforcedStyleForEmptyBrackets style enforces that
# empty reference brackets contain exactly one space.

# bad
foo[]
foo[    ]
foo[
]

# good
foo[ ]
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("no_space"),
                allowed: &["space", "no_space"],
                doc: "Whether reference brackets require or forbid surrounding space.",
            },
            ConfigOption {
                name: "EnforcedStyleForEmptyBrackets",
                default: ConfigDefault::Str("no_space"),
                allowed: &["space", "no_space"],
                doc: "Whether empty reference brackets (`[]`) require or forbid a single interior space.",
            },
        ],
        blind_spots: "\
`on_send` only ever fires for `[]`/`[]=` method calls, never a method
*definition* (`def Vector.[](*array)`, a distinct node kind) and never a
safe-navigated call (`a&.[](1)`, which RuboCop's own `on_send` is never
dispatched to either since this cop defines no `on_csend`); both are
excluded here the same way.

Ruby's `\\s` (whitespace) is modeled as ASCII space/tab/newline/CR/FF/VT;
no Unicode whitespace is treated as blank, matching MRI's own byte-based
lexer.

RuboCop's `autocorrect_with_disable_uncorrectable?` gate (the
`--disable-uncorrectable` CLI flag suppressing one side of a two-sided
offense) has no equivalent here: this engine does not model that CLI mode,
so both sides of a two-sided offense are always reported.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "space" => Style::Space,
            _ => Style::NoSpace,
        };
        let empty_style = match options.style("EnforcedStyleForEmptyBrackets")? {
            "space" => EmptyStyle::Space,
            _ => EmptyStyle::NoSpace,
        };
        Ok(Self { style, empty_style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some((open, close)) = brackets(node) else { return };
        check(node, open, close, self.style, self.empty_style, ctx);
    }
}
