# Status

Last updated: 2026-09-24. Phase 3 complete: all 50 rules landed.

## What works

- `elysium check [PATH|GLOB]...` parses every target with Prism (vendored
  `ruby-prism` 1.9.0, `partial_script: true` like RuboCop), walks the tree once
  through a static-dispatch `Dispatch` impl, and reports `Lint/Syntax` offenses
  with RuboCop-identical message text, line, and column (verified against
  `rubocop --format json` with `ParserEngine: parser_prism`, including
  multi-byte columns and same-range deduplication).
- Rules: 50 Style/Layout/Lint cops (see `docs/rules/`), each registered
  through `rule_set!` with a compile-time node-kind subscription table and
  configured from RuboCop option names (`RuleOptions`, incl. peer-cop and
  `AllCops` reads). `elysium fix [--unsafe] [--diff]` applies byte-range
  fixes and reparses to convergence; `check --only/--except` mirror RuboCop
  (`--only` force-enables a config-disabled cop).
- Fixtures: extracted from RuboCop's own specs by `tools/port_spec.rb`
  (instrumented `expect_offense`/`expect_correction`), checked for offenses,
  corrections, and fix idempotence by `crates/rules/tests/fixtures.rs`. A
  case's sibling `<case>.offenses` file replays a spec that built its cop
  with an explicit injected `offenses` array simulating diagnostics from
  other cops that never ran (`Lint/RedundantCopDisableDirective`'s
  cross-cop cases); `<case>.yml` supports whole-department overrides
  (`Department: {Enabled: false}`), not just per-cop options. Un-portable
  spec classes are deleted after generation and listed, with cause, in
  `crates/rules/fixtures/README.md` (see ADR 0006).
- Conformance: `cargo xtask conformance --app DIR --rule Cop` diffs offenses
  against real RuboCop on discourse/forem/mastodon/gitlab; results in
  `docs/conformance/rules.md`. All 50 rules are at 100% agreement on
  `discourse` and `mastodon` (RuboCop 1.91.0 truth) except
  `Lint/RedundantCopDisableDirective` (can't be measured the normal way —
  see below). `forem`'s remaining
  sub-100% rows are pinned to RuboCop 1.63.4 (its own `Gemfile.lock`
  version, run through a side Gemfile under rbenv 3.4.2 since forem's own
  `.ruby-version` targets Ruby 3.0); several are genuine RuboCop-version
  skew — `AllowQualifiedName` (`Layout/LineLength`) and
  `if_branch_is_true_type_and_else_is_not?` (`Style/RedundantCondition`)
  postdate 1.63.4, and 1.63.4's `Style/RedundantParentheses` still allows
  `:and` in `ALLOWED_NODE_TYPES` and lacks the newer
  `argument_of_parenthesized_method_call?`/`square_brackets?` checks — all
  documented per-row in `docs/conformance/rules.md`'s footnotes.
  `Lint/RedundantCopDisableDirective` cannot be measured through the xtask's
  normal `--only <cop>` recipe at all (RuboCop's CLI hard-refuses `--only`
  for this cop, exit 2, since its own logic depends on every other cop's
  reported offenses); a one-off full-lint-then-filter comparison instead
  shows agreement is dominated by disables of cops elysium doesn't
  implement yet, so the measurement is only meaningful once significantly
  more cops are ported. It stays at `nursery` regardless.
- Configuration additions from conformance work: extension gems'
  `config/default.yml` is loaded as a defaults layer (rubocop-rails'
  `bin/*` Exclude etc.), hidden directories are skipped like RuboCop's
  TargetFinder, `AllCops` filters apply once at discovery, and
  `# rubocop:disable-next|todo-next|enable-next` are recognised.
- Output: `-f human` (`path:line:col: F: Cop/Name: message` + summary) and
  `-f json` (RuboCop's JSON formatter schema, field for field).
  `AllCops/DisplayStyleGuide` and `AllCops/ExtraDetails` (and their
  `-S/--display-style-guide`/`-E/--extra-details` CLI flags) append each
  cop's resolved `StyleGuide`/`References`/`Details` annotation to its
  message, mirroring RuboCop's `MessageAnnotator`.
- `Alignment#display_column` (RuboCop's East Asian Width-aware rendered
  column) is one shared `SourceFile::display_column` helper in
  `ruby_source`, used by every alignment/indentation rule instead of a
  per-rule copy.
- Discovery: parallel walk honouring `.gitignore` (off with `--no-gitignore`),
  RuboCop's default `AllCops/Include` and `Exclude`, Ruby shebang detection for
  extensionless files, explicit files always linted, globs relative to cwd.
- Exit codes: 0 clean, 1 offenses, 2 I/O error.
- `--stats` prints discover/lint/output timings and node counts to stderr.
- Tooling: `cargo xtask bench --record|--check` (end-to-end over the corpus,
  fails on >5% regression), criterion micro-benchmarks (`cargo bench -p linter`),
  `cargo deny` config (all licenses permissive: MIT, Apache-2.0, BSD-3-Clause,
  ISC, Unicode-3.0), CI workflows for fmt/clippy pedantic/test/deny.
- `.rubocop.yml` loading: `inherit_from`/`inherit_gem` (gem paths resolved by
  filesystem search against `Gemfile.lock`, no Ruby/Bundler/RubyGems
  involved — see ADR 0005), `inherit_mode`, department-level switches,
  `DisabledByDefault`/`EnabledByDefault`, `Enabled: pending` plus `NewCops`,
  and `Exclude`/`Include` absolutisation relative to the file that declares
  them, matching `RuboCop::ConfigLoader`. Project root is discovered from
  `.rubocop.yml` location, not just the working directory.
- `elysium config [--format show-cops|yaml] [--only COP,...] [--config PATH]
  [--no-config]` prints the fully resolved configuration, in RuboCop's own
  `--show-cops`-compatible key order and format or as plain resolved YAML;
  `elysium check` gained the matching `--config PATH`/`--no-config` flags.
- `ruby_directives` parses `rubocop:disable|enable|todo` comments with exact
  line and end-of-line semantics and applies them in the engine, including
  `CommentConfig#cop_opted_in?`: a `# rubocop:enable Cop` directive naming a
  `DisabledByDefault`-disabled cop exactly, anywhere in the file, re-opts
  that cop in for that file (RuboCop's own `cop_opted_in?`/`comment_config`
  semantics), so a disable/enable pair on an otherwise-off cop is evaluated
  instead of silently ignored.
- Config conformance: `elysium config --format show-cops` matches
  `rubocop --show-cops` on three real Rails apps (discourse, forem,
  mastodon) 100% on `Enabled` state for every cop whose embedded default
  isn't itself stale relative to the app's pinned RuboCop version; the two
  remaining disagreements are documented RuboCop version skew, not
  elysium bugs (`docs/conformance/config.md`).

## What does not work yet

- `Lint/RedundantCopDisableDirective` cannot be conformance-measured through
  the xtask's normal per-rule harness (RuboCop's CLI rejects `--only` for
  this cop outright); its numbers so far come from a one-off manual
  full-lint comparison and are dominated by cops elysium hasn't implemented
  yet, so it stays at `nursery` until conformance is meaningful.
- No per-project RuboCop version model: rules follow 1.82.1 (and 1.91 where
  upstream reverted a default), so apps pinned to older RuboCop see skew
  (documented per-cop in `docs/conformance/rules.md`, e.g.
  `Layout/LineLength`'s `AllowQualifiedName` not existing in RuboCop 1.63).
- ERB embedded in `.rubocop.yml` is rejected with a clear error instead of
  evaluated (no Ruby runtime). This blocks loading GitLab's real
  `.rubocop.yml`; `--no-config` works around it there. See ADR 0005.
- Remote `inherit_from: https://...` is rejected instead of fetched.
- No `ConfigValidator`: a config with a wrong-typed value or an unknown cop
  name does not produce an error the way RuboCop's own validator does.
- `!ruby/regexp` YAML tags inside `Exclude`/`Include` entries are not
  matched; only plain glob-string entries work.
- Extension-gem cops (`Rails/*`, `RSpec/*`, ...) load their defaults from the
  installed gem but have no implementations yet (Phase 6).
- `TargetRubyVersion` is not inferred from a gemspec's `required_ruby_version`
  when the config doesn't set it explicitly.
- Syntax error message text matches RuboCop only under
  `ParserEngine: parser_prism`; the legacy `parser` engine wording
  (`unexpected token kEND`) is not reproduced. See ADR 0003.
- Encoding: files are treated as bytes; `# encoding:` magic comments other
  than UTF-8 are not honoured for column computation.

## Rule count by stability

| stable | preview | nursery |
|-------:|--------:|--------:|
| 49 | 0 | 1 |

Promotion to `stable` requires >99% corpus conformance on `discourse` and
`mastodon` (RuboCop 1.91 truth) with no unexplained diff; 49 of 50 rules meet
it. The one remaining at `nursery`:

- `Lint/RedundantCopDisableDirective` — held back per policy regardless of
  measured agreement (see above).

`Lint/Syntax` is built into the engine and is not counted.

## Benchmarks

### vs RuboCop (Phase 3, 49 shared cops)

Median of 3 runs each, three real Rails apps, RuboCop's own best case
(warm/cached) vs elysium's cold-every-time numbers. Full tables, exact
commands, rule list, and per-app CPU/RSS/offense breakdowns:
[`docs/benchmarks/phase3-vs-rubocop.md`](docs/benchmarks/phase3-vs-rubocop.md).

| app (files) | RuboCop cold `--parallel` | RuboCop warm `--parallel` | elysium (10 threads) | speedup vs cold | speedup vs warm |
|---|---:|---:|---:|---:|---:|
| discourse (12,120) | 104.72 s | 24.09 s | 2.18 s | 48.0x | 11.1x |
| mastodon (3,277) | 13.72 s | 5.36 s | 0.19 s | 72.2x | 28.2x |
| forem (2,987, RuboCop 1.63.4) | 27.48 s | 34.42 s | 0.29 s | 94.8x | 118.7x |

elysium has no result cache; RuboCop's warm column is its cache-primed best
case, and elysium still wins every row while re-parsing from scratch.

Host: Apple M4 (10 cores), macOS, release build (`lto = "fat"`).
Corpus: gitlab-foss `master`, 32,237 target files, 108.6 MB, 11.3M nodes.

End to end (`cargo xtask bench --record --runs 3 --rubocop`, median of 3,
`benchmarks/results.json`):

| benchmark | median | files |
|---|---:|---:|
| e2e/discover | 371 ms | - |
| e2e/lint (read + parse + walk) | 777 ms | - |
| e2e/total | 1,100 ms | 32,237 |
| rubocop/total (`--only Lint/Syntax`, RuboCop 1.82.1) | 132,464 ms | 32,231 |

Speedup: **120.4x** (`rubocop/total ÷ e2e/total`) for `Lint/Syntax` alone, the
only rule both tools currently run.

Discourse (12,133 target files, real `.rubocop.yml` with
`inherit_gem: rubocop-discourse`) lints end to end in 545 ms total.

File counts differ by 6 (elysium 32,237 vs RuboCop 32,231). All 6 are
extensionless scripts with a `ruby`/`rake` shebang under `vendor/gems/**`
(`vendor/gems/omniauth-salesforce/Rakefile`,
`vendor/gems/omniauth_crowd/Rakefile`, and four `vendor/gems/sidekiq/bin/*`
executables), which `AllCops/Exclude: vendor/**/*` drops for both tools.
elysium's shebang-detection fallback
(`crates/cli/src/discover.rs::ruby_shebang`) is only gated on the file
extension and never consulted through the `Exclude` matcher, so it adds
these 6 files back in; RuboCop's own shebang detection correctly honours
`Exclude`. This is a real elysium discovery bug (shebang detection should
run after, not instead of, the exclude check), not a benchmark artifact.

Phase 1 exit criterion (under 2 s cold on a 20k-file repository): met with
margin on a 32k-file repository. First run after a reboot-equivalent cold
disk cache measured 2.1 s total, dominated by 1.4 s of directory walking; the
parallel walker brought subsequent runs to the numbers above.

Micro (`cargo bench -p linter --bench pipeline`, criterion medians):

| fixture | bytes | parse | walk | lint |
|---|---:|---:|---:|---:|
| small (`label_link.rb`) | 1.5 KB | 10.8 µs | 0.41 µs | 10.8 µs |
| medium (`ability.rb`) | 8.2 KB | 43.2 µs | 1.7 µs | 41.6 µs |
| large (`user.rb`) | 123 KB | 927 µs | 53.5 µs | 966 µs |

Parsing is ~95% of per-file cost; the visitor walk is ~5%. Rule dispatch
will add to the walk share; parse cost is the floor.

Known risk: laptop run-to-run noise is 3–6%, which is within the 5% CI gate.
CI needs its own recorded baseline before `--check` is a hard gate
(`.github/workflows/bench.yml` runs it with `continue-on-error`).

## Decisions pending from the owner

- Project name (`elysium` is the working directory name) and license. No
  public commit until decided.

## Next three milestones

1. Phase 4: semantic layer (`ruby_semantic`: scopes and local variables) and
   the ten cops excluded from Phase 3 for needing it (`Style/RedundantSelf`,
   `Lint/UselessAssignment`, `Lint/ShadowedException`,
   `Lint/UselessAccessModifier`, `Lint/MissingSuper`,
   `Lint/ConstantResolution`, `Lint/ShadowingOuterLocalVariable`,
   `Lint/NumberConversion`, `Lint/SelfAssignment`,
   `Style/OptionalBooleanParameter` — see `docs/planning/phase3-rules.md`'s
   "Excluded as semantic" list).
2. Phase 5 `[INFERENCE — no dedicated planning doc yet, extrapolated from
   the "What does not work yet" list above]`: config/CLI hardening ahead of
   extension-gem cop work — a `ConfigValidator` (type/unknown-cop errors), a
   minimal ERB subset evaluator for `.rubocop.yml` (unblocking GitLab's real
   config), remote `inherit_from` fetching, `!ruby/regexp`
   `Include`/`Exclude` tags, `TargetRubyVersion` inference from a gemspec's
   `required_ruby_version`, and non-UTF-8 `# encoding:` column handling.
3. Phase 6: extension-gem cop implementations (`Rails/*`, `RSpec/*`,
   `Performance/*`, ...) — their `config/default.yml` layering and
   conformance skip-list (`EXTENSION_DEPARTMENTS`) already exist; only the
   cops themselves are unported.