# Cop porting kit

Everything a porter needs to turn one RuboCop cop into an elysium rule.
Read this instead of exploring the engine. Your cop's skeleton is already
created and registered, and its fixtures are already generated.

## Your job

1. Read the upstream cop: `$RUBOCOP/lib/rubocop/cop/<dept>/<cop>.rb`
   (`RUBOCOP=/Users/paulo/Work/lab/corpus/rubocop-1.91.0`); its spec is
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

## Extension cops (Rails, Performance, ThreadSafety, Minitest, Sorbet)

Same job, different places. Each gem is pinned in `tools/extension_gems.rb`
and has its own crate, `crates/rules_<key>` (`rules_rails`,
`rules_performance`, `rules_thread_safety`, `rules_minitest`,
`rules_sorbet`); fixtures still live in `crates/rules/fixtures/<key>/` and
run through the same harness.

| Gem | Source (`$GEM`) | Upstream test |
| --- | --- | --- |
| rubocop-rails | `/Users/paulo/Work/lab/corpus/rubocop-rails-2.38.0` | `spec/rubocop/cop/rails/<cop>_spec.rb` |
| rubocop-performance | `/Users/paulo/Work/lab/corpus/rubocop-performance-1.27.0` | `spec/rubocop/cop/performance/<cop>_spec.rb` |
| rubocop-thread_safety | `/Users/paulo/Work/lab/corpus/rubocop-thread_safety-0.8.0` | `spec/rubocop/cop/thread_safety/<cop>_spec.rb` |
| rubocop-minitest | `/Users/paulo/Work/lab/corpus/rubocop-minitest-0.40.0` | `test/rubocop/cop/minitest/<cop>_test.rb` |
| rubocop-sorbet | `/Users/paulo/Work/lab/corpus/rubocop-sorbet-0.16.0` | `test/rubocop/cop/sorbet/**/<cop>_test.rb` |

For a Rails cop (`Rails/ApplicationRecord`, snake `application_record`):

1. Read `$GEM/lib/rubocop/cop/rails/application_record.rb` (plus any mixin
   under `$GEM/lib/rubocop/cop/mixin/`), its spec, and its options in
   `crates/rules_rails/rubocop-rails/default.yml`.
2. Fill in `crates/rules_rails/src/rails/application_record.rs` only.
3. Iterate on
   `FIXTURE_COP=application_record cargo test -p rules --test fixtures -- --exact rails`.
4. Done when the fixtures pass and
   `cargo clippy -p rules_rails 2>&1 | grep -A5 'application_record.rs'` is
   empty.

For a Minitest cop (`Minitest/AssertNil`, snake `assert_nil`) the same with
`$GEM/lib/rubocop/cop/minitest/assert_nil.rb`,
`$GEM/test/rubocop/cop/minitest/assert_nil_test.rb`,
`crates/rules_minitest/rubocop-minitest/default.yml`,
`crates/rules_minitest/src/minitest/assert_nil.rs`,
`FIXTURE_COP=assert_nil cargo test -p rules --test fixtures -- --exact minitest`
and `cargo clippy -p rules_minitest 2>&1 | grep -A5 'assert_nil.rs'`.
ThreadSafety's department directory is `thread_safety`
(`-- --exact thread_safety`). Messages are upstream's verbatim, including
mixin messages (`NilAssertionHandleable::MSG` and the like).

What differs from core cops:

- `minimum_target_rails_version N` (Rails): RuboCop skips the cop when
  the target is below `N`, so the rule reports nothing then. The target is
  `options.peer("AllCops", "TargetRailsVersion")`, which is `null` unless
  set; unset means 5.0 in the fixtures (the specs stub `railties` at 5.0).
  `TargetRailsVersion: 4.2` in a case's `.yml` comes from `:rails42`.
- `requires_gem 'rack', '>= 3.1.0'` (Rails and friends): RuboCop skips the
  cop unless the target's lockfile satisfies it. `Config#gem_versions_in_target`
  is `LoadedConfig::gem_versions()`: the `Gemfile.lock` (else `gems.locked`)
  found upward from the config's base directory, every locked gem included;
  `None` without a config file or lockfile. Rules reach it through
  `RuleOptions`: `options.requires_gem("rack", &[">= 3.1.0"])` is the gate
  (false when there is no lockfile or the gem is absent, so keep the cop
  inert then), `options.gem_version("rack")` is `target_gem_version`
  (`Option<GemVersion>`, comparable, from `linter::GemVersion`; requirements
  are `linter::GemRequirement`, Gemfile syntax incl. `~>`), and
  `options.target_rails_version()` is rubocop-rails'
  `TargetRailsVersion.resolve`: `AllCops/TargetRailsVersion`, else the
  lockfile's `railties` major.minor, else 5.0. Fixtures: a spec's stubbed
  `let(:gem_versions) { { 'rack' => '3.1.0' } }` becomes a
  `# gem_versions: rack=3.1.0` comment line in the case `.yml` (written by
  `port_spec.rb`, read by the harness, which gives every case an empty
  lockfile otherwise). `railties` is not recorded: `:rails42` and friends
  already set `AllCops: TargetRailsVersion`.
- Minitest suites parse at Ruby 3.4, so `target_ruby_version()` is 3.4 in
  minitest/sorbet fixtures unless the `.yml` says otherwise.
- A sorbet case whose `.yml` has `AllCops: DisplayCopNames: true` expects
  `Sorbet/Foo: ` before each message; the harness adds that prefix. Report
  the plain upstream `MSG`.

The integrator creates the skeleton and fixtures exactly as for core:

```sh
ruby tools/scaffold_cop.rb Rails/ApplicationRecord Minitest/AssertNil
ruby tools/port_spec.rb --cop Rails/ApplicationRecord \
  --rubocop-src /Users/paulo/Work/lab/corpus/rubocop-1.91.0 --out crates/rules/fixtures
ruby tools/port_spec.rb --cop Minitest/AssertNil \
  --rubocop-src /Users/paulo/Work/lab/corpus/rubocop-1.91.0 --out crates/rules/fixtures
```

`port_spec.rb` finds each gem's checkout next to `--rubocop-src`
(`<gem>-<version>`); pass `--gem-src rails=PATH,minitest=PATH` for any other
location.

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
        severity: Severity::Warning,        // default.yml `Severity:`, else Lint = Warning, else Convention
        fix: FixAvailability::Safe,         // None | Safe | Unsafe (see rule.rs)
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
