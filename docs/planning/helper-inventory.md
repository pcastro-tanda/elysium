# Helper Inventory — crates/rules/src/{style,layout,lint}

(Written by a read-only scout before the 80/20 port; head groups are now in `ruby_source`, `ruby_ast::ext` and `linter`. The tail list is the pending backfill tracked in `STATUS.md`.)

## Ranked duplicate-helper groups (head = ≥5 rule files)

| # | Group | Proposed home | Files | Straight swap? |
|---|-------|---------------|-------|-----------------|
| 1 | `Node#single_line?` / `multiline?` (RuboCop-AST) | `linter::mixins::multiline` | guard_clause, redundant_parentheses (as `is_multiline`), empty_lines_around_class_body, else_layout, hash_alignment, trailing_comma (already `pub is_multiline_span`), trailing_comma_in_array_literal, trailing_comma_in_hash_literal, empty_line_between_defs (9) | Straight swap — all take `(ctx, span) -> bool`, same first/last-line comparison. |
| 2 | `RangeHelp`-style horizontal/full whitespace span expansion (`range_with_surrounding_space`, `Util#swallow_*`) | `linter::mixins::range_help` | accessor_grouping (`span_with_leading_space_removed`), sole_nested_conditional (`horizontal_space_span`,`full_space_span`), space_around_operators (`extend_left`,`extend_right`), space_inside_block_braces (`extend_forward`,`extend_backward`), space_inside_array_literal_brackets (`reposition_forward/backward`,`skip_blank_forward/backward`), extra_spacing (`prev_visible_end`), redundant_cop_disable_directive (`swallow_right`,`swallow_left`) (7) | Needs adaptation — same walk-while-blank-byte loop, but some also swallow newlines (`include_newlines` flag) and some walk left, some right; unify into one bidirectional fn with `(newlines: bool, dir: Left/Right)`. |
| 3 | Heredoc-node detection (RuboCop-AST `HeredocNode`/`Node#heredoc?`) | `linter::mixins::heredoc` (needs `ctx.text` on opening_loc) | guard_clause (`string_heredoc_locs`,`heredoc_branch`), if_unless_modifier (`heredoc_regions`), trailing_comma_in_hash_literal (`is_heredoc_value`,`heredoc_opening_span`,`is_heredoc_call`), trailing_comma_in_array_literal (`is_heredoc`), trailing_comma (already `pub is_heredoc`/`pub any_heredoc`), first_hash_element_indentation (`heredoc_taboo`), line_length (`is_heredoc_node`) (7) | Straight swap for the boolean check (`is_heredoc(node) -> bool`); `heredoc_taboo`/`heredoc_branch` need a thin per-rule wrapper on top. |
| 4 | `Util#begins_its_line?` | `linter::mixins::range_help::begins_its_line` | argument_alignment, first_argument_indentation, indentation_consistency, indentation_width, trailing_comma_in_array_literal, trailing_comma (6) | Straight swap — identical `(ctx, span) -> bool` logic in every copy. |
| 5 | `AlignmentCorrector`-style multiline shift-edit builder (shift every non-blank line of a span by `column_delta`, respecting heredoc/taboo ranges) | `linter::mixins::alignment_corrector` | argument_alignment (`build_fix`), first_argument_indentation (`build_shift_fix`), first_hash_element_indentation (`build_shift_fix`), indentation_consistency (`build_fix`), indentation_width (`build_alignment_edits`), class_and_module_children (`build_alignment_edits`) (6) | Needs light adaptation — same skeleton (per-line delta insert/delete of leading whitespace, skip blank lines when indenting, `Edit`/`Fix` construction) but callers pass different taboo-range sets (heredoc bodies vs `=begin/=end`). Parameterize with a taboo-ranges slice. |
| 6 | Ruby `\s` whitespace byte/char predicate (`is_ruby_whitespace`/`is_ruby_ws_byte`/`is_regex_ws`) | `ruby_source` (pure, no Context) | argument_alignment, first_hash_element_indentation, indentation_consistency, first_argument_indentation (byte+char variants), if_unless_modifier (`is_regex_ws`), documentation (`is_regex_ws`) (6) | Straight swap — all match the same byte set `{' ','\t','\n','\r',0x0B,0x0C}` (some char, some byte; expose both). |
| 7 | `trim_start` (strip leading ASCII whitespace bytes) | `ruby_source` | argument_alignment, first_argument_indentation, first_hash_element_indentation, indentation_consistency, indentation_width (5) | Straight swap — byte-identical implementation in all 5 files. |
| 8 | Leading-whitespace / first-non-blank-column-of-line reimplementations (duplicates existing `Context::display_column` / `ctx.line_text`) | Discoverability fix: call existing `Context::display_column`/`line_text`, no new module needed | class_and_module_children (`leading_whitespace`,`columns_of`), argument_alignment (`indentation_of_line`), extra_spacing (`line_indentation`), space_around_operators (`line_indentation`), first_hash_element_indentation (`first_non_ws_column`), first_argument_indentation (`char_indent_of`) (6) | Not a swap into a *new* home — these should call the *existing* `Context::display_column`/`line_text` instead of re-deriving column math. Flagged separately in "already-shared, re-implemented" section below. |
| 9 | `Range#last_line` (line containing a span's last byte) | `linter::mixins::range_help::last_line_of` | accessor_grouping (`last_line`), empty_lines_around_class_body (`last_line_of`), first_hash_element_indentation (`last_line_of`), trailing_comma_in_array_literal (`last_line_of`), trailing_comma (`last_line`) (5) | Straight swap — identical `ctx.line_col(span.end.saturating_sub(1).max(span.start)).line` in every copy. |

## Head group detail

### 1. `is_single_line`/`is_multiline` — RuboCop-AST `Node#single_line?`
```rust
fn is_single_line(ctx: &Context<'_>, span: Span) -> bool {
    let end = span.end.saturating_sub(1).max(span.start);
    ctx.line_col(span.start).line == ctx.line_col(end).line
}
```
Compares a span's start/end line; every one of the 9 copies is this exact three-liner (sometimes phrased as its negation, `is_multiline`).

### 2. RangeHelp span expansion — RuboCop-AST `RangeHelp#range_with_surrounding_space`
Walk a byte offset left/right while the byte is a plain space/tab (optionally also `\n`), returning the new offset. 7 files each hand-roll one or two directions of this walk for their own local use (extra spacing detection, brace/bracket space checks, disable-directive comment removal).

### 3. Heredoc-node detection — RuboCop-AST `HeredocNode`
```rust
fn is_heredoc(node: &Node<'_>) -> bool {
    // match on StringNode/InterpolatedStringNode/... opening_loc(),
    // true iff ctx.text(opening) starts_with(b"<<")
}
```
Every string-kind rule that has to skip or specially handle heredocs re-derives this from Prism's per-kind `opening_loc()` accessor.

### 4. `begins_its_line?` — RuboCop `Util#begins_its_line?`
```rust
fn begins_its_line(ctx: &Context<'_>, span: Span) -> bool {
    let lc = ctx.line_col(span.start);
    let line = ctx.line_text(lc.line);
    // true iff only blank bytes precede span.start on lc.line
}
```

### 5. AlignmentCorrector shift-edit builder
Given a multiline node and a signed column delta, walks every line of the node's span and emits an `Edit` that inserts/deletes leading whitespace, skipping blank lines on indent and skipping any line inside a supplied "taboo" range (heredoc bodies, `=begin/=end` blocks). Six files each reimplement this ~40-60 line routine nearly verbatim, differing mainly in how they compute the taboo ranges.

### 6. Ruby whitespace predicate
```rust
fn is_ruby_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\r' | '\x0B' | '\x0C')
}
```
(Byte-oriented sibling `is_ruby_ws_byte`/`is_regex_ws` match the same set on `u8`.) Pure function, no Context needed — cheapest possible port.

### 7. `trim_start`
```rust
fn trim_start(text: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < text.len() && text[i].is_ascii_whitespace() { i += 1; }
    &text[i..]
}
```
Byte-identical across 5 files (Rust's `[u8]::trim_ascii_start` was apparently avoided/unavailable at MSRV; worth checking if it can just be replaced by std once stabilized).

### 8. Leading-whitespace/column reimplementations (discoverability gap, not a new module)
`Context` already has `display_column(offset)` (RuboCop's `Alignment#display_column`) and `line_text(line)`, yet 6 files independently re-scan a line for its first non-whitespace byte/column instead of calling these. This is pure re-derivation of data the Context already computes — the fix is call-site migration, not a new shared function.

### 9. `Range#last_line`
```rust
fn last_line_of(ctx: &Context<'_>, span: Span) -> u32 {
    ctx.line_col(span.end.saturating_sub(1).max(span.start)).line
}
```

## Tail groups (2-4 rule files — lower priority, listed for completeness)

- **`lambda_or_proc?`/`Proc.new` detection** (`is_proc_const`+`is_lambda_or_proc`, byte-identical) — ambiguous_block_association, empty_block, symbol_proc (3 files). Home: `ruby_ast::ext` (pure Node predicate, no Context).
- **`call_span_excluding_block`/`call_end_excluding_block`** (byte-identical) — ambiguous_block_association, debugger (2 files). Home: `ruby_ast::ext`.
- **Bare access-modifier detection** (`is_bare_access_modifier`/`is_visibility_block`, `private`/`protected`/`public`/`module_function` with no receiver) — accessor_grouping, empty_lines_around_class_body, indentation_consistency, indentation_width (indentation_width even has two near-duplicate copies in the same file: `is_bare_access_modifier` and `is_special_modifier`) (4 files). Home: `ruby_ast::ext`.
- **`Util#comment_line?`** (`/^\s*#/` on a line) — accessor_grouping, empty_lines_around_class_body, first_argument_indentation (3 files). Home: `ruby_source` (pure bytes-in variant exists in accessor_grouping).
- **`range_by_whole_lines(range, include_final_newline: true)`** — guard_clause, if_unless_modifier, sole_nested_conditional (3 files, byte-identical `whole_lines_span`). Home: `linter::mixins::range_help`.
- **UTF-8 char counting** (`char_len`/`char_count`/`byte_to_char_col`, Ruby `String#length` semantics) — if_unless_modifier, first_hash_element_indentation, line_length, trailing_whitespace (4 files). Home: `ruby_source`.
- **`recursive_basic_literal?`/literal-NodeKind classifiers** — redundant_parentheses (`literal_kind`,`numeric_kind`,`const_kind`,`variable_kind`), mutable_constant (`is_mutable_literal`,`is_immutable_literal`), duplicate_hash_key (`recursive_basic_literal`) (3 files, different shapes of the same underlying classification). Home: `ruby_ast::ext`, but needs real adaptation — each rule's literal boundary differs slightly (RuboCop's own `recursive_basic_literal?` is one concept, but redundant_parentheses/mutable_constant intentionally use narrower/wider variants for their own semantics).
- **`is_bare_or_toplevel_const`** (`(const {nil? cbase} :Name)` matcher) — mutable_constant, duplicate_methods (2 files, byte-identical). Home: `ruby_ast::ext`.
- **`names_within`** (binary-search a span-sorted `(Span, name)` list for names inside a range) — guard_clause, if_unless_modifier (2 files, byte-identical, ~6 lines). Home: `ruby_ast::ext` or a small `linter` utility.
- **`same_line(ctx, a, b)` span-on-same-line check** — first_argument_indentation, indentation_width, hash_alignment (as `is_single_line`-adjacent), first_hash_element_indentation (4 files, slightly different signatures: `(Span,Span)` vs `(u32,u32)`). Home: `linter::mixins::range_help`.

## Already-shared code that rules re-implement anyway (discoverability problem)

- `Context::display_column` exists and matches RuboCop's `Alignment#display_column` exactly, but 6 rule files (group 8 above) independently re-scan lines for indentation/column instead of calling it.
- `crates/rules/src/style/trailing_comma.rs` already exports `pub fn is_multiline_span`, `pub fn is_heredoc`, and `pub fn any_heredoc` for the three `trailing_comma_in_*` siblings, but sibling files outside that family (guard_clause, if_unless_modifier, line_length, first_hash_element_indentation) re-derive equivalent single-line/heredoc checks locally rather than promoting `trailing_comma`'s helpers to a shared module both families can use. This is the clearest existing precedent for where a `linter::mixins::{multiline,heredoc}` module should live — it can be built by lifting `trailing_comma`'s existing three functions out, not by writing new ones.
- No `ruby_ast::ext` module exists at all despite `ruby_ast::lib.rs`'s own doc comment saying it deliberately keeps Prism's generated node structs un-wrapped "because [they're] already zero-cost and well named" — that design choice is fine for accessors but leaves no natural home for cross-rule *predicates* (heredoc?, lambda_or_proc?, bare_access_modifier?, recursive_basic_literal?), so every rule grows its own copy inline instead of reaching for a shared one.

## Rank draw

Head/tail line drawn at 5 files: groups 1-9 above (5-9 files each) are head; the 10 tail groups (2-4 files each) are lower priority but still worth batching into the same PR since several (`ruby_ast::ext` predicates) are one-line ports.

## Estimate & most-affected files

Counting only the head groups' duplicate bodies (excluding the one canonical copy each group would keep), the removable/consolidatable line count is roughly 800-900 lines [INFERENCE: extrapolated from observed per-copy sizes of 3-60 lines across the 9 head groups × 5-9 duplicate copies each], concentrated overwhelmingly in the `layout/` indentation-and-alignment family, which is also where the `AlignmentCorrector`-style shift-edit builder (group 5, ~40-60 lines per copy) lives.

Most-affected rule files (by number of distinct duplicate groups they contain):
1. `layout/first_hash_element_indentation.rs` — 8 groups (build_shift_fix, is_ruby_whitespace, trim_start, last_line_of, same_line, first_non_ws_column, char_len, heredoc_taboo)
2. `layout/first_argument_indentation.rs` — 7 groups (begins_its_line, build_shift_fix, is_ruby_ws_byte/char, trim_start, char_indent_of, same_line, is_comment_only_line)
3. `layout/indentation_width.rs` — 5 groups (begins_its_line, build_alignment_edits, trim_start, is_bare_access_modifier×2, same_line, column_of) — tied with `layout/argument_alignment.rs` and `layout/indentation_consistency.rs` at 5 groups each; indentation_width is called out because it additionally contains two near-duplicate access-modifier predicates *within the same file*.