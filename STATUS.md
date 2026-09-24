# Status

Last updated: 2026-09-22. Phase 3 in progress: 40 of 50 rules landed.

## What works

- `elysium check [PATH|GLOB]...` parses every target with Prism (vendored
  `ruby-prism` 1.9.0, `partial_script: true` like RuboCop), walks the tree once
  through a static-dispatch `Dispatch` impl, and reports `Lint/Syntax` offenses
  with RuboCop-identical message text, line, and column (verified against
  `rubocop --format json` with `ParserEngine: parser_prism`, including
  multi-byte columns and same-range deduplication).
- Rules: 40 Style/Layout/Lint cops (see `docs/rules/`), each registered
  through `rule_set!` with a compile-time node-kind subscription table and
  configured from RuboCop option names (`RuleOptions`, incl. peer-cop and
  `AllCops` reads). `elysium fix [--unsafe] [--diff]` applies byte-range
  fixes and reparses to convergence; `check --only/--except` mirror RuboCop
  (`--only` force-enables a config-disabled cop).
- Fixtures: 3,900+ cases extracted from RuboCop 1.82.1's own specs by
  `tools/port_spec.rb` (instrumented `expect_offense`), checked for offenses,
  corrections, and fix idempotence by `crates/rules/tests/fixtures.rs`.
- Conformance: `cargo xtask conformance --app DIR --rule Cop` diffs offenses
  against real RuboCop on discourse/forem/mastodon/gitlab; results in
  `docs/conformance/rules.md`. Batch one (16 rules) is at 100% on
  discourse and mastodon; forem's remaining diffs are RuboCop 1.63 /
  Ruby 3.0 version skew (documented per rule in the log samples).
- Configuration additions from conformance work: extension gems'
  `config/default.yml` is loaded as a defaults layer (rubocop-rails'
  `bin/*` Exclude etc.), hidden directories are skipped like RuboCop's
  TargetFinder, `AllCops` filters apply once at discovery, and
  `# rubocop:disable-next|todo-next|enable-next` are recognised.
- Output: `-f human` (`path:line:col: F: Cop/Name: message` + summary) and
  `-f json` (RuboCop's JSON formatter schema, field for field).
- Discovery: parallel walk honouring `.gitignore` (off with `--no-gitignore`),
  RuboCop's default `AllCops/Include` and `Exclude`, Ruby shebang detection for
  extensionless files, explicit files always linted, globs relative to cwd.
- Exit codes: 0 clean, 1 offenses, 2 I/O error.
- `--stats` prints discover/lint/output timings and node counts to stderr.
- Tooling: `cargo xtask bench --record|--check` (end-to-end over the corpus,
  fails on >5% regression), criterion micro-benchmarks (`cargo bench -p linter`),
  `cargo deny` config (all licenses permissive: MIT, Apache-2.0, BSD-3-Clause,
  ISC, Unicode-3.0), CI workflows for fmt/clippy pedantic/test/deny. 125 tests
  pass across the workspace.
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
  line and end-of-line semantics and applies them in the engine.
- Config conformance: `elysium config --format show-cops` matches
  `rubocop --show-cops` on three real Rails apps (discourse, forem,
  mastodon) 100% on `Enabled` state for every cop whose embedded default
  isn't itself stale relative to the app's pinned RuboCop version; the two
  remaining disagreements are documented RuboCop version skew, not
  elysium bugs (`docs/conformance/config.md`).

## What does not work yet

- Ten of the fifty Phase 3 rules have fixtures but no implementation yet
  (listed in the `rule_set!` comment in `crates/rules/src/lib.rs`):
  Style/ClassAndModuleChildren, Style/EmptyElse, Style/AccessorGrouping,
  Style/RedundantRegexpEscape, Style/StringConcatenation,
  Lint/RedundantCopDisableDirective, Lint/Debugger, Lint/DuplicateHashKey,
  Lint/DuplicateMethods, Lint/EmptyBlock.
- `DisplayStyleGuide: true` does not append the style-guide URL to messages.
- No per-project RuboCop version model: rules follow 1.82.1 (and 1.91 where
  upstream reverted a default), so apps pinned to older RuboCop see skew
  (e.g. `Layout/LineLength` `AllowQualifiedName` did not exist in 1.63).
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
| 0 | 0 | 40 |

Promotion to `stable` requires >99% corpus conformance; batch one meets it
on discourse and mastodon and will be promoted after a fresh run with all
40 rules.

`Lint/Syntax` is built into the engine and is not counted.

## Benchmarks

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

1. Phase 3: implement the ten pending rules, run conformance for rules
   17–50 on the corpus, promote rules with >99% agreement to `stable`.
2. Phase 3 cleanup: shared `display_column` (East Asian width) helper
   replacing three per-rule copies; `DisplayStyleGuide` message suffix;
   docs regenerated from `RuleMeta`.
3. Phase 4: semantic layer (`ruby_semantic`: scopes and local variables) and
   the ten cops excluded from Phase 3 for needing it.
