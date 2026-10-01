# Plan: elysium ready to replace RuboCop on payaus

Goal: `elysium check` on payaus reports exactly what RuboCop 1.91.0 (plus the
extension gems) reports under payaus's config, before anything in payaus
changes. Payaus is only read, from throwaway worktrees.

## Starting point (measured 2026-09-30)

- Target: payaus under RuboCop 1.91.0 and the current extension gems, with
  only the config edits the bump forces (`ThreadSafety/InstanceVariableInClassMethod`
  renamed to `ThreadSafety/ClassInstanceVariable`) plus decision C. Its
  enabled cops are `docs/planning/waves/payaus.txt`: 812 (core 504,
  extension 250, Tanda custom 59). Elysium implements 344 of them.
- On that target elysium loads the whole config (inherit chain, local
  `require:` Ruby files are recorded and ignored without noise) and lints
  22,281 files in 9 s; `conformance-config` agrees on 790/790 core cops.
- Of the implemented cops, 13 disagreed with RuboCop on payaus although
  discourse/mastodon are at 100%, and the directive single-line rule added
  74 `Lint/MissingCopEnableDirective` offenses (step 1).

## Steps

### 0. Target list — done
`docs/planning/waves/payaus.txt`, resolved by RuboCop itself
(`cop_enabled?` over the registry) in a throwaway payaus worktree.

### 1. Engine, config, and payaus conformance of implemented cops — done
Result (payaus `847ad4df7fb` with the target config): every one of the 344
implemented cops in the target list reports exactly RuboCop 1.91.0's
offenses, over the identical 22,282-file set; elysium 11 s, RuboCop `-P` 155 s.

- `!ruby/regexp` in `Include`/`Exclude` (AllCops, department, cop), routed
  like RuboCop (AllCops `Exclude` on the absolute path, a cop's `Exclude` on
  the relative then the absolute path, `Include` on the relative path).
- `Lint/CopDirectiveSyntax` (default-enabled since 1.91; payaus disables it).
- `DirectiveComment#single_line?`: a directive that doesn't start its
  comment (`# typed: false # rubocop:disable Sorbet/TrueSigil`, 74 payaus
  files) is single-line; `ruby_directives` had approximated this by "code
  before the comment".
- 13 cops with payaus-only differences fixed: `Layout/IndentationWidth`,
  `RescueEnsureAlignment`, `SpaceInsideParens`, `SpaceBeforeFirstArg`,
  `ExtraSpacing`, `SpaceAroundOperators`; `Style/RedundantParentheses`,
  `SafeNavigation`, `TernaryParentheses`, `MultilineIfModifier`;
  `Lint/SafeNavigationChain`, `MixedRegexpCaptureTypes`; `Naming/FileName`.
- `tools/port_spec.rb` records the peer-cop keys a cop reads and states the
  spec's nil values; `RescueEnsureAlignment`'s `BeginEndAlignment` default
  had been invisible to the fixtures (one removed case restored).
- File set: `config/initializers/02_configuration/secret_token.rb` is
  tracked but listed in `.gitignore`; ADR 0004 skips it, RuboCop lints it.
  Payaus runs `elysium check --no-gitignore` (exact RuboCop file set).
- ERB: none in elysium; payaus replaces the ERB block with the static list
  (decision C).

### 2. Remaining core cops payaus enables (~3-4 waves)
158 cops: 123 pending (Lint 43, Style 69, Gemspec 5, Layout 4, Security 2),
13 opt-in (`Layout/ClassStructure`, the 5 `First*LineBreak`/
`MultilineHashKeyLineBreaks`, `Style/AutoResourceCleanup`, `CollectionMethods`,
`DateTime`, `MethodCallWithArgsParentheses`, `MultilineMethodSignature`,
`ReturnNil`, `Send`), and 22 added in 1.83-1.91 (Lint 7, Style 15). Same
pipeline as waves 3-11: fixtures from 1.91.0 specs, conformance on
discourse/mastodon and payaus.
`Style/DoubleCopDisableDirective` is obsolete in 1.91 and is not ported.

### 3. Extension cops (~5 waves)
250 cops payaus enables: Rails 115, Minitest 49, Performance 41, Sorbet 36,
ThreadSafety 9. Prerequisite: extend `tools/port_spec.rb`/`check_fixtures.rb`
to each gem's spec suite, pinned to the current releases (decision A: rails
2.38.0, performance 1.27.0, minitest 0.40.0, sorbet 0.16.0, thread_safety
0.8.0), and the CI pin job to those gems. Payaus bumps the gems alongside
RuboCop 1.91 at cutover. Rails first (largest, and discourse/mastodon give
corpus truth for it); Sorbet and Minitest have no corpus besides payaus.

### 4. Tanda custom cops (1-2 waves)
59 cops, ~3,800 lines of Ruby in `payaus/rubocop/custom_cops`, ported into a
new Tanda department in `crates/rules` (decision B).
Fixtures from payaus's own `test/rubocop` tests where they exist (20 files).

### 5. Payaus acceptance
Truth cache from RuboCop on a payaus worktree with the target config; run
`cargo xtask conformance` for every cop in the target list. Ready = 100% on
all of them, plus identical file set (Include/Exclude, the Rails-department
exclusion from `rubocop/patches/disable_rails_cops.rb`).

## Decisions (2026-09-30)

- A. Extension versions: current releases (2.38.0, 1.27.0, 0.40.0, 0.16.0, 0.8.0).
- B. Custom cops: in elysium.
- C. ERB: payaus switches to the static 80-acronym list at cutover.

## Payaus changes at cutover (only after step 5 passes)
RuboCop 1.91.0 and current extension gems; static acronym list; the Rails
department `Exclude` replacing `rubocop/patches/disable_rails_cops.rb`;
fixes for the offenses the bump introduces (491 core with the 25 new
1.91 cops disabled, measured 2026-09-30, plus whatever the gem bumps add).

## Out of scope
`--format progress`, a `rubocop`-compatible entry point, result cache
(`CacheRootDirectory`, `MaxFilesInCache`) — per 2026-09-30 instruction.
