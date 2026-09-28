# Cop porting kit

Everything a porter needs to turn one RuboCop cop into an elysium rule.
Read this instead of exploring the engine. Your cop's skeleton is already
created and registered, and its fixtures are already generated.

## Your job

1. Read the upstream cop: `$RUBOCOP/lib/rubocop/cop/<dept>/<cop>.rb`
   (`RUBOCOP=/Users/paulo/Work/lab/corpus/rubocop-1.82.1`); its spec is
   `$RUBOCOP/spec/rubocop/cop/<dept>/<cop>_spec.rb`, its options are in
   `$RUBOCOP/config/default.yml`.
2. Fill in `crates/rules/src/<dept>/<cop>.rs` (the skeleton). Edit **only
   that file**. Do not touch `lib.rs`, `mod.rs`, fixtures, other rules, or
   shared crates. If you truly need a shared helper, copy it privately into
   your file.
3. Iterate on:

   ```sh
   FIXTURE_COP=<cop_snake> cargo test -p rules --test fixtures -- --exact <dept>
   ```

   Only failures under `fixtures/<dept>/<cop_snake>/` are yours; ignore
   other cops. `UPDATE_FIXTURES` is forbidden. Never edit fixtures.
4. Done when your fixtures pass and
   `cargo clippy -p rules 2>&1 | grep -A5 '<cop_snake>.rs'` is empty. Do not
   run the workspace test suite, conformance, fmt, or benchmarks; the
   integrator does that once per batch.
5. If a fixture case is impossible to satisfy (a Prism/whitequark AST
   difference, not a bug), say which case and why in your final answer. Do
   not delete it.

Keep the port literal: same conditions, same message strings, same offense
ranges as upstream. No extra heuristics.

## Rule shape

```rust
use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix,
    FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str = "Upstream MSG verbatim.";

#[derive(Debug, Clone)]
pub struct MyCop { allow_comments: bool }

impl Rule for MyCop {
    const META: RuleMeta = RuleMeta {
        name: "Lint/MyCop",
        department: Department::Lint,
        summary: "…",                       // default.yml Description
        explanation: "…",                   // upstream class doc, markdown
        enabled_by_default: true,
        severity: Severity::Warning,        // Lint = Warning, else Convention
        fix: FixAvailability::Always,       // None | Always | Sometimes (see rule.rs)
        stability: Stability::Nursery,      // keep Nursery
        kinds: &[NodeKind::WhenNode],       // nodes you get enter/leave for
        config: &[ConfigOption {
            name: "AllowComments",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "…",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_comments: options.bool("AllowComments") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(when) = node.as_when_node() else { return };
        let span = when.keyword_loc().span();
        ctx.report(&Self::META, span, MSG);
    }
}
```

Check `crates/linter/src/rule.rs` for exact `FixAvailability`/`ConfigDefault`
variants if unsure. Good small examples to copy from:
`lint/float_out_of_range.rs` (node), `lint/empty_file.rs` (file-level),
`layout/empty_comment.rs` (comments), `style/accessor_grouping.rs` (fixes),
`layout/space_inside_parens.rs` (token/space scanning with fixes).

Hooks: `file_start(ctx)`, `enter(node, ctx)`, `leave(node, ctx)`,
`file_end(ctx)`. A rule is cloned per file, so per-file state in `self` is
fine. Line/comment-only cops set `kinds: &[]` and work in `file_end`.

## Options (`RuleOptions`)

`bool(k)`, `int(k)`, `str(k)`, `str_list(k)`, `style(k)` (validated
against `allowed`, returns `Err` for unsupported values), `get(k)`,
`target_ruby_version() -> f32` (fixtures default to 3.3), `peer(cop, k)` for
other cops' options.

## Context (`crates/linter/src/context.rs`)

- Text: `source()`, `text(span) -> &[u8]`, `line_col(off)` (1-based line,
  0-based byte column), `line_text(line)`, `line_span(line)`,
  `line_count()`, `lines()`, `display_column(off)`.
- Geometry: `is_single_line(span)`, `same_line(a, b)`, `last_line(span)`,
  `begins_its_line(span)`, `whole_lines(span)`, `with_surrounding_space(..)`.
- Tree: `parent() -> Option<NodeInfo>`, `ancestors() -> &[NodeInfo]`
  (`NodeInfo` has `kind` and `span`), `depth()`, `parsed().root()`.
- Comments: `comments() -> &[CommentInfo]` (`span`, `kind`:
  `CommentKind::{Inline, EmbDoc}`), `parsed().magic_comments()`,
  `directives()` (rubocop:disable/enable parsing).
- `opaque_spans()` / `in_opaque_span(off)`: strings, heredocs, regexps,
  comments -- skip these when scanning raw bytes.
- `semantics()`: scopes/variables, only if the cop needs VariableForce.
- Reporting: `report(&Self::META, span, msg)`,
  `report_with_fix(&Self::META, span, msg, Fix { applicability:
  Applicability::Safe, edits: vec![Edit::replace(span, b"x".to_vec())] })`,
  `Edit::delete(span)`, `Edit::insert(offset, text)`,
  `report_global(&Self::META, msg)` for `add_global_offense` (renders as
  `^{}` in fixtures). Unsafe autocorrect = `Applicability::Unsafe`.

`Span { start, end }` are `u32` byte offsets; `Span::new(a, b)`,
`Span::empty(off)`. Use `u32::try_from(x).expect("offset exceeds u32")`
rather than `as u32` (clippy pedantic).

## Prism API

Node accessors for every node type, one line each:
`docs/porting/prism-api.txt` (grep it, e.g. `grep '^CaseNode:'`). Generic
`Node` has `as_<snake>_node()`, `kind()`, `span()`, `location()`.
`ConstantId::as_slice() -> &[u8]` for names. `Location::span()` via
`LocationExt`. Walk helpers in `ruby_ast`: `for_each_child`,
`each_descendant`, `walk` + `Visitor`. Helpers in `ruby_ast::ext`:
`is_heredoc`, `is_lambda_or_proc`, `is_bare_access_modifier`,
`call_span_excluding_block`, `top_receiver`, `const_name`.

## whitequark → Prism traps

RuboCop's AST (parser gem) differs from Prism. Known mappings:

- `send`/`csend` → `CallNode`; `csend` is `is_safe_navigation()`. Most
  `on_send` cops do *not* fire on `&.` unless upstream aliases `on_csend`.
- `block` wraps the call in whitequark; in Prism the `BlockNode` is the
  call's `block()`. A `&blk` argument is `BlockArgumentNode` in `block()`,
  not a `BlockNode`. `numblock`/`itblock` = `BlockNode` with numbered/`it`
  parameters.
- `begin` with `rescue` is `:rescue`/`:ensure` inside `:kwbegin`; in Prism a
  `BeginNode` with `rescue_clause()`/`ensure_clause()`. Method bodies with
  rescue are a `BeginNode` as `def.body()` with no `begin_keyword_loc`.
- whitequark elides single-statement `begin`; Prism always has
  `StatementsNode`. `(begin ...)` for parens is `ParenthesesNode`.
- `if`/`unless`/ternary/modifier: Prism has `IfNode` and `UnlessNode`;
  ternary = `IfNode` with no `if_keyword_loc`; `elsif` = `IfNode` in
  `subsequent()`; `else` = `ElseNode`.
- `multiline?` is `!ctx.is_single_line(node.span())`; for blocks upstream
  measures the block node only (`BlockNode` span starts at `{`/`do`).
- Assignments: `lvasgn`/`ivasgn`/... = `LocalVariableWriteNode` etc.;
  `op_asgn` = `*OperatorWriteNode`; `or_asgn`/`and_asgn` = `*OrWriteNode` /
  `*AndWriteNode`; `masgn` = `MultiWriteNode`.
- `str`/`dstr` = `StringNode`/`InterpolatedStringNode`; `sym`/`dsym`
  similar; heredocs are strings with `opening_loc` starting `<<`
  (`ext::is_heredoc`).
- `hash` vs keyword args: braceless keyword args are `KeywordHashNode`.
- `const` = `ConstantReadNode` / `ConstantPathNode`; `cbase` is a
  `ConstantPathNode` with no `parent()`.
- `args` = `ParametersNode` (can be absent); `arg`/`optarg`/`restarg`/... =
  `RequiredParameterNode`/`OptionalParameterNode`/`RestParameterNode`/...
- `node.loc.expression` of a `rescue` clause differs: Prism's `RescueNode`
  starts at the `rescue` keyword.
- Offense column/length: fixtures annotate with `^`; a mismatch of a few
  columns is almost always the wrong `Location`.
