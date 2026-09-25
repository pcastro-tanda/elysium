//! `Layout/SpaceInsideArrayLiteralBrackets`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_inside_array_literal_brackets.rb` plus the
//! `SurroundingSpace` mixin (`lib/rubocop/cop/mixin/surrounding_space.rb`)
//! and `SpaceCorrector` (`lib/rubocop/cop/correctors/space_corrector.rb`) it
//! uses.
//!
//! RuboCop's mixin reasons over a *token stream*: `left_bracket?`/
//! `right_bracket?` tokens, `space_after?`/`space_before?` (does the
//! character immediately outside a token match `\s`?), and `new_line?`
//! tokens (`tNL`) it explicitly skips over when walking from one bracket to
//! the next ("is the neighboring *token*, ignoring pure line breaks,
//! another bracket?"). Since RuboCop's lexer never tokenizes plain
//! horizontal whitespace at all, "the next real token" and "the next
//! non-whitespace byte" coincide exactly, so every token-stream query below
//! is re-derived as a byte scan over the raw source instead:
//!
//! * `extra_space?` (a plain space/tab immediately adjacent) -> byte
//!   equality against `' '`/`'\t'`.
//! * `Token#space_after?`/`#space_before?` (*any* whitespace, incl.
//!   newlines) -> [`any_space_after`]/[`any_space_before`].
//! * `multi_dimensional_array?` (skip `tNL` tokens, is the next real token a
//!   bracket?) -> [`multi_dim_left`]/[`multi_dim_right`], which skip any run
//!   of whitespace/newlines and check the next byte. A line comment
//!   sitting in that gap stops the scan at its leading `#`, which is never
//!   a bracket, so it is (like RuboCop's own comment token) correctly never
//!   treated as adjacent.
//! * `next_to_comment?` -> [`next_is_comment`].
//! * `next_to_newline?` (empirically: true exactly when the rest of the
//!   bracket's own line is blank, i.e. the first element starts on the next
//!   line) -> [`starts_on_next_line`].
//! * `end_has_own_line?` (no non-whitespace before the bracket on its own
//!   line) -> the shared `Context::begins_its_line`.
//!
//! Prism folds a pattern's leading constant directly into
//! [`ArrayPatternNode`]/[`FindPatternNode`] (`ADT[a, b]` is one node with a
//! `constant` field and its own `opening_loc`/`closing_loc`), so RuboCop's
//! `find_node_with_brackets` ancestor search -- needed only because
//! whitequark wraps a bracketed/parenthesized constant pattern in a
//! separate `const_pattern` node -- has no Prism equivalent to port: each
//! node already carries its own brackets (or, for a paren-style `ADT(a,
//! b)`/parenthesized pattern, an `opening_loc` that isn't literally `[`,
//! filtered out below exactly like a `%w[]` array literal is).
use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::Node;
use ruby_ast::{LocationExt, NodeExt, NodeKind};
use ruby_source::{is_ruby_whitespace, Span};

/// RuboCop's `MSG`, formatted with `SPACE_COMMAND`.
const MSG_USE: &str = "Use space inside array brackets.";
/// RuboCop's `MSG`, formatted with `NO_SPACE_COMMAND`.
const MSG_NO_USE: &str = "Do not use space inside array brackets.";
/// RuboCop's `EMPTY_MSG`, formatted with `'Use one'`.
const MSG_EMPTY_USE_ONE: &str = "Use one space inside empty array brackets.";
/// RuboCop's `EMPTY_MSG`, formatted with `NO_SPACE_COMMAND`.
const MSG_EMPTY_NO_USE: &str = "Do not use space inside empty array brackets.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    NoSpace,
    Space,
    Compact,
}

/// RuboCop's `EnforcedStyleForEmptyBrackets`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmptyStyle {
    NoSpace,
    Space,
}

/// `SurroundingSpace::SINGLE_SPACE_REGEXP`: plain space or tab only, never a
/// newline (which is instead handled by the multiline `start_ok`/`end_ok`
/// checks).
const fn is_space_or_tab(b: u8) -> bool {
    matches!(b, b' ' | b'\t')
}

/// `Token#space_after?`-alike: is the byte right after `pos` any whitespace?
fn any_space_after(bytes: &[u8], pos: u32) -> bool {
    bytes.get(pos as usize).is_some_and(|&b| is_ruby_whitespace(b))
}

/// `Token#space_before?`-alike: is the byte right before `pos` any whitespace?
fn any_space_before(bytes: &[u8], pos: u32) -> bool {
    pos > 0 && is_ruby_whitespace(bytes[pos as usize - 1])
}

/// `extra_space?(token, :left)`-alike: a plain space/tab right after `pos`.
fn extra_space_after(bytes: &[u8], pos: u32) -> bool {
    bytes.get(pos as usize).is_some_and(|&b| is_space_or_tab(b))
}

/// `extra_space?(token, :right)`-alike: a plain space/tab right before `pos`.
fn extra_space_before(bytes: &[u8], pos: u32) -> bool {
    pos > 0 && is_space_or_tab(bytes[pos as usize - 1])
}

/// `SurroundingSpace#reposition(src, pos, +1, ...)`: extends `pos` forward
/// over a run of plain spaces/tabs (plus newlines when `include_newlines`).
fn reposition_forward(bytes: &[u8], mut pos: u32, include_newlines: bool) -> u32 {
    while let Some(&b) = bytes.get(pos as usize) {
        if is_space_or_tab(b) || (include_newlines && b == b'\n') {
            pos += 1;
        } else {
            break;
        }
    }
    pos
}

/// `SurroundingSpace#reposition(src, pos, -1, ...)`: extends `pos` backward
/// over a run of plain spaces/tabs (plus newlines when `include_newlines`).
fn reposition_backward(bytes: &[u8], mut pos: u32, include_newlines: bool) -> u32 {
    while pos > 0 {
        let b = bytes[pos as usize - 1];
        if is_space_or_tab(b) || (include_newlines && b == b'\n') {
            pos -= 1;
        } else {
            break;
        }
    }
    pos
}

/// Skips every whitespace byte (space/tab/newline/...) forward from `pos`,
/// standing in for walking past a run of token-stream `tNL`s: real tokens
/// (including comments) are never whitespace, so this always lands exactly
/// on the next real token's first byte.
fn skip_blank_forward(bytes: &[u8], mut pos: u32) -> u32 {
    while bytes.get(pos as usize).is_some_and(|&b| is_ruby_whitespace(b)) {
        pos += 1;
    }
    pos
}

/// Backward counterpart of [`skip_blank_forward`].
fn skip_blank_backward(bytes: &[u8], mut pos: u32) -> u32 {
    while pos > 0 && is_ruby_whitespace(bytes[pos as usize - 1]) {
        pos -= 1;
    }
    pos
}

/// `multi_dimensional_array?(node, left, side: :left)`: skipping whitespace,
/// is the next byte after the left bracket itself a left bracket?
fn multi_dim_left(bytes: &[u8], open_end: u32) -> bool {
    let pos = skip_blank_forward(bytes, open_end);
    bytes.get(pos as usize) == Some(&b'[')
}

/// `multi_dimensional_array?(node, right)`: skipping whitespace, is the byte
/// before the right bracket itself a right bracket?
fn multi_dim_right(bytes: &[u8], close_start: u32) -> bool {
    let pos = skip_blank_backward(bytes, close_start);
    pos > 0 && bytes[pos as usize - 1] == b']'
}

/// `next_to_comment?`: skipping whitespace, is the next byte after the left
/// bracket a comment's leading `#`?
fn next_is_comment(bytes: &[u8], open_end: u32) -> bool {
    let pos = skip_blank_forward(bytes, open_end);
    bytes.get(pos as usize) == Some(&b'#')
}

/// `next_to_newline?`: is the rest of the left bracket's own line -- skipping
/// only plain horizontal whitespace -- blank, i.e. does a newline arrive
/// before any other byte (comment or element)?
fn starts_on_next_line(bytes: &[u8], open_end: u32) -> bool {
    let mut pos = open_end as usize;
    loop {
        match bytes.get(pos) {
            Some(b' ' | b'\t' | b'\r') => pos += 1,
            Some(b'\n') => return true,
            _ => return false,
        }
    }
}

/// Resolves the `[`/`]` bracket spans of an array literal, array pattern, or
/// find pattern, restricted (like RuboCop-AST's `square_brackets?`) to a
/// literal single-byte `[` opening -- excluding `%w[]`-style array literals
/// (whose "opening" is the whole `%w[` prefix) and paren-style patterns
/// (`ADT(a, b)`, opening `(`). Bare/bracket-less patterns (`foo in 1, 2`)
/// have no opening/closing location at all and are skipped the same way
/// RuboCop's own token search comes up empty for them.
fn brackets(node: &Node<'_>) -> Option<(Span, Span)> {
    let (opening, closing) = match node.kind() {
        NodeKind::ArrayNode => {
            let array = node.as_array_node()?;
            (array.opening_loc()?, array.closing_loc()?)
        }
        NodeKind::ArrayPatternNode => {
            let pattern = node.as_array_pattern_node()?;
            (pattern.opening_loc()?, pattern.closing_loc()?)
        }
        NodeKind::FindPatternNode => {
            let pattern = node.as_find_pattern_node()?;
            (pattern.opening_loc()?, pattern.closing_loc()?)
        }
        _ => return None,
    };
    if opening.as_slice() != b"[" {
        return None;
    }
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
    ctx.report_with_fix(&SpaceInsideArrayLiteralBrackets::META, span, message, fix);
}

/// `no_space_offenses`, left side.
fn no_space_left(open: Span, bytes: &[u8], start_ok: bool, ctx: &mut Context<'_>) {
    if start_ok || !extra_space_after(bytes, open.end) {
        return;
    }
    let span = Span::new(open.end, reposition_forward(bytes, open.end, false));
    emit(ctx, span, MSG_NO_USE, span, b"");
}

/// `no_space_offenses`, right side.
fn no_space_right(close: Span, bytes: &[u8], end_ok: bool, ctx: &mut Context<'_>) {
    if end_ok || !extra_space_before(bytes, close.start) {
        return;
    }
    let span = Span::new(reposition_backward(bytes, close.start, false), close.start);
    emit(ctx, span, MSG_NO_USE, span, b"");
}

/// `space_offenses`, left side. Also RuboCop's `compact_offenses`'
/// non-multi-dimensional left branch (`space_offenses(node, left, nil, ...,
/// end_ok: true)`, which reduces to this same left-only check).
fn space_left(open: Span, bytes: &[u8], start_ok: bool, ctx: &mut Context<'_>) {
    if start_ok || extra_space_after(bytes, open.end) {
        return;
    }
    emit(ctx, open, MSG_USE, Span::new(open.end, open.end), b" ");
}

/// `space_offenses`, right side. Also `compact_offenses`' non-multi-
/// dimensional right branch.
fn space_right(close: Span, bytes: &[u8], end_ok: bool, ctx: &mut Context<'_>) {
    if end_ok || extra_space_before(bytes, close.start) {
        return;
    }
    emit(ctx, close, MSG_USE, Span::new(close.start, close.start), b" ");
}

/// `compact_offenses`, left side.
fn compact_left(open: Span, bytes: &[u8], start_ok: bool, ctx: &mut Context<'_>) {
    if multi_dim_left(bytes, open.end) {
        if !any_space_after(bytes, open.end) {
            return;
        }
        // The diagnostic (like RuboCop's) only covers a plain space/tab
        // run, possibly zero-width when a bare newline separates the
        // brackets; the fix additionally collapses that newline.
        let diag = Span::new(open.end, reposition_forward(bytes, open.end, false));
        let fix = Span::new(open.end, reposition_forward(bytes, open.end, true));
        emit(ctx, diag, MSG_NO_USE, fix, b"");
    } else {
        space_left(open, bytes, start_ok, ctx);
    }
}

/// `compact_offenses`, right side.
fn compact_right(close: Span, bytes: &[u8], end_ok: bool, ctx: &mut Context<'_>) {
    if multi_dim_right(bytes, close.start) {
        if !any_space_before(bytes, close.start) {
            return;
        }
        let diag = Span::new(reposition_backward(bytes, close.start, false), close.start);
        let fix = Span::new(reposition_backward(bytes, close.start, true), close.start);
        emit(ctx, diag, MSG_NO_USE, fix, b"");
    } else {
        space_right(close, bytes, end_ok, ctx);
    }
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

/// `on_array`/`on_array_pattern`: dispatches to the empty-brackets check or
/// the style-specific left/right checks.
fn check(open: Span, close: Span, style: Style, empty_style: EmptyStyle, ctx: &mut Context<'_>) {
    let bytes = ctx.source().bytes();
    let between = Span::new(open.end, close.start);
    if bytes[between.range()].iter().all(|&b| is_ruby_whitespace(b)) {
        check_empty(open, close, empty_style, bytes, ctx);
        return;
    }

    let single_line = ctx.same_line(open, close);
    let end_ok = !single_line && ctx.begins_its_line(Span::new(close.start, close.start));

    match style {
        Style::NoSpace => {
            let start_ok = next_is_comment(bytes, open.end);
            no_space_left(open, bytes, start_ok, ctx);
            no_space_right(close, bytes, end_ok, ctx);
        }
        Style::Space => {
            let start_ok = starts_on_next_line(bytes, open.end);
            space_left(open, bytes, start_ok, ctx);
            space_right(close, bytes, end_ok, ctx);
        }
        Style::Compact => {
            let start_ok = starts_on_next_line(bytes, open.end);
            compact_left(open, bytes, start_ok, ctx);
            compact_right(close, bytes, end_ok, ctx);
        }
    }
}

/// Checks that brackets used for array literals (and array/find patterns)
/// have or don't have surrounding space depending on configuration.
#[derive(Debug, Clone)]
pub struct SpaceInsideArrayLiteralBrackets {
    style: Style,
    empty_style: EmptyStyle,
}

impl Rule for SpaceInsideArrayLiteralBrackets {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceInsideArrayLiteralBrackets",
        department: Department::Layout,
        summary: "Checks the spacing inside array literal brackets.",
        explanation: "\
Checks that brackets used for array literals have or don't have
surrounding space depending on configuration.

Array pattern matching (`in [1, 2]`, `Const[1, 2]`) is handled the same way.

```ruby
# EnforcedStyle: no_space (default)
# The `no_space` style enforces that array literals have
# no surrounding space.

# bad
array = [ a, b, c, d ]
array = [ a, [ b, c ]]

# good
array = [a, b, c, d]
array = [a, [b, c]]
```

```ruby
# EnforcedStyle: space
# The `space` style enforces that array literals have
# surrounding space.

# bad
array = [a, b, c, d]
array = [ a, [ b, c ]]

# good
array = [ a, b, c, d ]
array = [ a, [ b, c ] ]
```

```ruby
# EnforcedStyle: compact
# The `compact` style normally requires a space inside
# array brackets, with the exception that successive left
# or right brackets are collapsed together in nested arrays.

# bad
array = [a, b, c, d]
array = [ a, [ b, c ] ]
array = [
  [ a ],
  [ b, c ]
]

# good
array = [ a, b, c, d ]
array = [ a, [ b, c ]]
array = [[ a ],
  [ b, c ]]
```

```ruby
# EnforcedStyleForEmptyBrackets: no_space (default)
# The `no_space` EnforcedStyleForEmptyBrackets style enforces that
# empty array brackets do not contain spaces.

# bad
foo = [ ]
bar = [     ]

# good
foo = []
bar = []
```

```ruby
# EnforcedStyleForEmptyBrackets: space
# The `space` EnforcedStyleForEmptyBrackets style enforces that
# empty array brackets contain exactly one space.

# bad
foo = []
bar = [    ]

# good
foo = [ ]
bar = [ ]
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ArrayNode, NodeKind::ArrayPatternNode, NodeKind::FindPatternNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("no_space"),
                allowed: &["space", "no_space", "compact"],
                doc: "Whether array literal brackets require, forbid, or collapse surrounding space.",
            },
            ConfigOption {
                name: "EnforcedStyleForEmptyBrackets",
                default: ConfigDefault::Str("no_space"),
                allowed: &["space", "no_space"],
                doc: "Whether empty array literal brackets (`[]`) require or forbid a single interior space.",
            },
        ],
        blind_spots: "\
`multi_dimensional_array?`/`next_to_comment?`/`next_to_newline?` are
re-derived as raw byte scans over source text rather than RuboCop's token
stream (see the module doc for the equivalence argument). The one case this
does not model exactly: a line comment sitting directly between two
brackets whose own text happens to end in `[` or `]` (e.g. `] # ]`) would
be misread as bracket-adjacent by a naive scan; this implementation instead
stops at the comment's leading `#` (never a bracket), same as RuboCop's own
token-adjacency check, so no divergence is expected in practice.

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
            "compact" => Style::Compact,
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
        check(open, close, self.style, self.empty_style, ctx);
    }
}
