# Status

Last updated: 2026-09-18. Phase 1 complete.

## What works

- `elysium check [PATH|GLOB]...` parses every target with Prism (vendored
  `ruby-prism` 1.9.0, `partial_script: true` like RuboCop), walks the tree once
  through a static-dispatch `Dispatch` impl, and reports `Lint/Syntax` offenses
  with RuboCop-identical message text, line, and column (verified against
  `rubocop --format json` with `ParserEngine: parser_prism`, including
  multi-byte columns and same-range deduplication).
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
  ISC, Unicode-3.0), CI workflows for fmt/clippy pedantic/test/deny.

## What does not work yet

- No `.rubocop.yml` loading, no inline `rubocop:disable` directives (Phase 2).
- Zero lint rules beyond `Lint/Syntax`; `RuleSet` is an empty dispatcher and
  the registry codegen is not written yet (Phase 3).
- Project root is the working directory. `Exclude` patterns are root-relative
  (as in RuboCop), so running from a parent directory does not exclude
  `vendor/**/*`. Phase 2 sets root from `.rubocop.yml` discovery.
- Syntax error message text matches RuboCop only under
  `ParserEngine: parser_prism`; the legacy `parser` engine wording
  (`unexpected token kEND`) is not reproduced. See ADR 0003.
- Encoding: files are treated as bytes; `# encoding:` magic comments other
  than UTF-8 are not honoured for column computation.

## Rule count by stability

| stable | preview | nursery |
|-------:|--------:|--------:|
| 0 | 0 | 0 |

`Lint/Syntax` is built into the engine and is not counted.

## Benchmarks

Host: Apple M4 (10 cores), macOS, release build (`lto = "fat"`).
Corpus: gitlab-foss `master`, 32,237 target files, 108.6 MB, 11.3M nodes.

End to end (`cargo xtask bench`, median of 5, `benchmarks/results.json`):

| benchmark | median |
|---|---:|
| e2e/discover | 325 ms |
| e2e/lint (read + parse + walk) | 571 ms |
| e2e/total | 898 ms |

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

1. Phase 2: `.rubocop.yml` loader with `inherit_from`/`inherit_gem`, root
   discovery, `Include`/`Exclude` per cop, `config` subcommand; conformance
   against `rubocop --show-cops` on three apps.
2. Phase 2: `ruby_directives` parsing `rubocop:disable|enable|todo` with exact
   line/end-of-line semantics, applied in the engine.
3. Phase 3: rule registry codegen (`Dispatch` from `META.kinds`), fixture
   snapshot harness, and the first ten rules by real-world frequency with the
   conformance runner comparing against RuboCop on the corpus.
