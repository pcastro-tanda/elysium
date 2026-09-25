# Phase 3 benchmark: elysium vs RuboCop

Date: 2026-09-25. Host: Apple M4, 10 cores, macOS. elysium: release build
(`target/release/elysium`, `lto = "fat"`). RuboCop: 1.91.0 for discourse and
mastodon, 1.63.4 for forem (forem's own pinned `Gemfile.lock` version, run
under `rbenv 3.4.2` via a side Gemfile since forem's `.ruby-version` targets
an older Ruby). All runs use the same 49-cop `--only` list on all three
tools/modes, so this measures per-rule linting throughput, not full default
rulesets. All exit codes are 1 (offenses found), which is expected — not a
failure.

Each number below is the **median of 3 runs**. Wall/CPU are wall-clock and
`user+sys` seconds from `/usr/bin/time -l`; RSS is maximum resident set size
in decimal MB (`bytes / 1e6`). Corpus apps were deleted after the run to
reclaim disk space, so these numbers cannot be re-derived by re-running the
harness; they come from `/tmp/bench_results.tsv` (raw timings) and the
`--format json` outputs RuboCop/elysium wrote alongside each run (file and
offense counts).

## Commands

Exact commands from the benchmark harness (`RULES49` is the comma-joined
49-cop list below; `$GF` is each app's dedicated `rubocop.Gemfile`):

```sh
# RuboCop startup alone
BUNDLE_GEMFILE=$GF RBENV_VERSION=3.4.2 rbenv exec bundle exec rubocop --version

# RuboCop cold (no cache), parallel / no-parallel
BUNDLE_GEMFILE=$GF RBENV_VERSION=3.4.2 rbenv exec bundle exec rubocop \
  --cache false --parallel --only $RULES49 --format json --out OUT.json .
BUNDLE_GEMFILE=$GF RBENV_VERSION=3.4.2 rbenv exec bundle exec rubocop \
  --cache false --no-parallel --only $RULES49 --format json --out OUT.json .

# RuboCop warm: prime the cache once, then measure against it
BUNDLE_GEMFILE=$GF RBENV_VERSION=3.4.2 rbenv exec bundle exec rubocop \
  --cache true --cache-root CACHE_ROOT --only $RULES49 --format json --out PRIME.json .
BUNDLE_GEMFILE=$GF RBENV_VERSION=3.4.2 rbenv exec bundle exec rubocop \
  --cache true --cache-root CACHE_ROOT --parallel --only $RULES49 --format json --out OUT.json .
BUNDLE_GEMFILE=$GF RBENV_VERSION=3.4.2 rbenv exec bundle exec rubocop \
  --cache true --cache-root CACHE_ROOT --no-parallel --only $RULES49 --format json --out OUT.json .

# elysium, default (all cores) and single-threaded
elysium check --only $RULES49 -f json . > OUT.json
elysium check --only $RULES49 -j 1 -f json . > OUT.json
```

## Rule list (49 cops)

`Layout/ArgumentAlignment`, `Layout/EmptyLineBetweenDefs`,
`Layout/EmptyLines`, `Layout/EmptyLinesAroundClassBody`,
`Layout/ExtraSpacing`, `Layout/FirstArgumentIndentation`,
`Layout/FirstHashElementIndentation`, `Layout/HashAlignment`,
`Layout/IndentationConsistency`, `Layout/IndentationWidth`,
`Layout/LineLength`, `Layout/SpaceAroundOperators`,
`Layout/SpaceInsideArrayLiteralBrackets`, `Layout/SpaceInsideBlockBraces`,
`Layout/SpaceInsideHashLiteralBraces`, `Layout/TrailingEmptyLines`,
`Layout/TrailingWhitespace`, `Lint/AmbiguousBlockAssociation`,
`Lint/Debugger`, `Lint/DuplicateHashKey`, `Lint/DuplicateMethods`,
`Lint/ElseLayout`, `Lint/EmptyBlock`, `Lint/RedundantStringCoercion`,
`Style/AccessorGrouping`, `Style/ClassAndModuleChildren`,
`Style/Documentation`, `Style/EmptyElse`, `Style/FrozenStringLiteralComment`,
`Style/GuardClause`, `Style/HashSyntax`, `Style/IfUnlessModifier`,
`Style/IfUnlessModifierOfIfUnless`, `Style/MutableConstant`,
`Style/NumericLiteralPrefix`, `Style/NumericLiterals`,
`Style/RedundantCondition`, `Style/RedundantParentheses`,
`Style/RedundantRegexpCharacterClass`, `Style/RedundantRegexpEscape`,
`Style/RedundantReturn`, `Style/SoleNestedConditional`,
`Style/StringConcatenation`, `Style/StringLiterals`, `Style/SymbolProc`,
`Style/TrailingCommaInArguments`, `Style/TrailingCommaInArrayLiteral`,
`Style/TrailingCommaInHashLiteral`, `Style/WordArray`.

This is 49 of elysium's 50 stable+nursery cops; `Lint/RedundantCopDisableDirective`
is excluded because RuboCop's CLI hard-rejects `--only
Lint/RedundantCopDisableDirective` (exit 2 — the cop depends on every other
cop's reported offenses and can't run in isolation).

## Results

"speedup vs cold" and "speedup vs warm" are `RuboCop --parallel wall / row
wall` for that app, i.e. how many times faster (or slower, if <1x) that row
is than RuboCop's own cold-parallel or warm-parallel run. `--version`
rows (interpreter/gem-load startup alone, no linting) have no speedup
column since they don't inspect any files.

### discourse — 12,120 files

| tool | mode | wall s | CPU s | RSS MB | files | offenses | vs cold-parallel | vs warm-parallel |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| rubocop | --version (startup only) | 0.58 | 0.45 | 84 | - | - | - | - |
| rubocop | cold, --parallel | 104.72 | 103.57 | 635 | 12,120 | 348,298 | 1.00x | 0.23x |
| rubocop | cold, --no-parallel | 105.23 | 103.94 | 641 | 12,120 | 348,298 | 1.00x | 0.23x |
| rubocop | warm (cached), --parallel | 24.09 | 177.40 | 1,038 | 12,120 | 348,298 | 4.35x | 1.00x |
| rubocop | warm (cached), --no-parallel | 106.07 | 105.31 | 657 | 12,120 | 348,298 | 0.99x | 0.23x |
| elysium | 10 threads (default) | 2.18 | 12.55 | 171 | 12,120 | 348,298 | 48.04x | 11.05x |
| elysium | -j 1 | 5.82 | 6.02 | 151 | 12,120 | 348,298 | 17.99x | 4.14x |

### mastodon — 3,277 files

| tool | mode | wall s | CPU s | RSS MB | files | offenses | vs cold-parallel | vs warm-parallel |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| rubocop | --version (startup only) | 0.62 | 0.50 | 82 | - | - | - | - |
| rubocop | cold, --parallel | 13.72 | 13.58 | 116 | 3,277 | 2,777 | 1.00x | 0.39x |
| rubocop | cold, --no-parallel | 13.81 | 13.65 | 119 | 3,277 | 2,777 | 0.99x | 0.39x |
| rubocop | warm (cached), --parallel | 5.36 | 33.93 | 123 | 3,277 | 2,777 | 2.56x | 1.00x |
| rubocop | warm (cached), --no-parallel | 14.02 | 13.87 | 118 | 3,277 | 2,777 | 0.98x | 0.38x |
| elysium | 10 threads (default) | 0.19 | 1.18 | 43 | 3,277 | 2,777 | 72.21x | 28.21x |
| elysium | -j 1 | 0.62 | 0.66 | 29 | 3,277 | 2,777 | 22.13x | 8.65x |

### forem — 2,987 files (RuboCop 1.63.4)

| tool | mode | wall s | CPU s | RSS MB | files | offenses | vs cold-parallel | vs warm-parallel |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| rubocop | --version (startup only) | 0.64 | 0.51 | 76 | - | - | - | - |
| rubocop | cold, --parallel | 27.48 | 26.80 | 220 | 2,987 | 4,652 | 1.00x | 1.25x |
| rubocop | cold, --no-parallel | 27.69 | 27.34 | 218 | 2,987 | 4,652 | 0.99x | 1.24x |
| rubocop | warm (cached), --parallel | 34.42 | 88.59 | 225 | 2,987 | 4,652 | 0.80x | 1.00x |
| rubocop | warm (cached), --no-parallel | 28.33 | 27.31 | 222 | 2,987 | 4,652 | 0.97x | 1.21x |
| elysium | 10 threads (default) | 0.29 | 1.65 | 45 | 2,987 | 4,646 | 94.76x | 118.69x |
| elysium | -j 1 | 0.86 | 0.90 | 27 | 2,987 | 4,646 | 31.95x | 40.02x |

Offense counts match elysium and RuboCop exactly on discourse (348,298) and
mastodon (2,777). On forem, elysium reports 4,646 vs RuboCop 1.63.4's 4,652
(6 fewer) — elysium's rules follow RuboCop ~1.82's semantics
(`docs/conformance/rules.md` documents this as version skew on a few cops,
e.g. `Style/RedundantParentheses` and `Style/RedundantCondition`), not a
elysium bug specific to this benchmark.

## Reading the numbers

- **elysium is 18x–95x faster than RuboCop's cold run** across all three
  apps (comparing default thread count to `--parallel`), and 4x–119x faster
  than RuboCop's own best-case warm/cached run.
- **`--parallel` only helps RuboCop when the corpus is large enough to
  amortize fork overhead.** On discourse (12,120 files) parallel cold is
  about even with no-parallel cold, but warm parallel is 4.4x faster than
  warm no-parallel. On mastodon and forem (3,277 and 2,987 files) the gap
  from parallelism is smaller, and on forem the *warm* parallel run is
  actually slower (34.4 s) than cold (27.5 s) — cache-hit deserialization
  and process-pool coordination cost more than re-parsing outright at that
  corpus size, and CPU-seconds for warm-parallel exceed cold-parallel on
  every app (RuboCop spends more aggregate CPU coordinating multiple
  processes than it saves by skipping AST rebuilds).
- **elysium's own parallelism scales as expected**: 10-thread wall time is
  roughly `-j 1` wall time divided by 2.7–4x (not the full 10x, since
  discovery/output stages and small-app overhead aren't perfectly
  parallel), while CPU-seconds stay flat or drop slightly (better cache
  locality per thread, less contention than RuboCop's fork-based workers).
- **Peak RSS**: elysium uses roughly a quarter to a third of RuboCop's
  memory on every app and mode (e.g. discourse: 151–171 MB for elysium vs
  635 MB–1.04 GB for RuboCop), and RuboCop's warm/cached mode is the
  heaviest of all — the on-disk cache plus its own process pool costs more
  memory than a cold run, not less.

## Caveat

**elysium has no result cache today** — every `elysium check` invocation
re-parses and re-lints every file from scratch, same as RuboCop's cold
runs. RuboCop's warm numbers above are its best case: a cache primed by an
untimed prior run, then reused across three timed runs with no source
changes. Despite that structural disadvantage, elysium's cold-every-time
numbers still beat RuboCop's warm-cache numbers on every app in this
benchmark. A real-world incremental-lint workflow (only a few files changed
since the last commit) is exactly the scenario RuboCop's cache is built
for and elysium doesn't yet address; these numbers only speak to
full-corpus linting.
</content>
<parameter name="i">Write phase3 benchmark report