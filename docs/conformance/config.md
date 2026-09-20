# Config conformance: `elysium config` vs. real RuboCop

`cargo xtask conformance-config --app <checkout> --truth <app>.show-cops.yml` runs
`elysium config --format show-cops` inside a real Rails checkout and diffs it, cop
by cop, against a `rubocop --show-cops` capture taken from that same checkout with
its real `Gemfile.lock`-pinned RuboCop. A cop present on only one side (added,
removed, or renamed between the embedded defaults' RuboCop release and the release
that produced the capture) is "version skew" and excluded from the agreement
percentages; a cop in a RuboCop extension gem elysium does not embed yet (Rails,
Performance, RSpec, Capybara, FactoryBot, Rake) is "extension-department" and
likewise excluded. `Include`/`Exclude` are compared with each app's own absolute
root stripped from every entry first, since both `--show-cops` runs absolutise
those patterns against the checkout they ran in.

| App | RuboCop version | Cops compared | Enabled agreement | All-keys agreement | Skew count |
| --- | --- | --- | --- | --- | --- |
| discourse | 1.91.0 | 598 | 598/598 (100.0%) | 508/598 (84.9%) | 47 (+279 extension-department) |
| forem | 1.63.4 | 549 | 548/549 (99.8%) | 471/549 (85.8%) | 48 (+311 extension-department) |
| mastodon | 1.91.0 | 592 | 591/592 (99.8%) | 505/592 (85.3%) | 38 (+320 extension-department) |

"Cops compared" is the agreement denominator: cops present on both sides, outside
a skipped extension department. `Enabled agreement` is the metric
`conformance-config` gates on by default (`--report-only` was used to still print
disagreements for the two apps that fall short); `All-keys agreement` additionally
requires every other resolved key (`Include`, `Exclude`, `AllowedMethods`, etc.) to
match and is reported for completeness only.

## Remaining `Enabled` disagreements

Every remaining `Enabled` disagreement is version skew: the embedded defaults come
from RuboCop 1.82.1, and the corresponding cop's default value changed at a RuboCop
release the embedded defaults don't cover, or the capture predates the RuboCop
release the embedded defaults track.

- **forem: `Lint/ShadowingOuterLocalVariable`** (elysium `Enabled: false`, truth
  `Enabled: true`). RuboCop 1.82.1's `config/default.yml` shows
  `VersionChanged: '1.76'` for this cop, i.e. its default was flipped from `true`
  to `false` at RuboCop 1.76; forem's capture is RuboCop 1.63.4 (which predates
  1.76 and reports no `VersionChanged` for the cop at all), so it correctly still
  shows the old default.
- **mastodon: `Lint/CopDirectiveSyntax`** (elysium `Enabled: pending`, truth
  `Enabled: true`). RuboCop's `CHANGELOG.md` for 1.91.0 lists
  "[#15616](https://github.com/rubocop/rubocop/pull/15616): Enable
  `Lint/CopDirectiveSyntax` by default, so directives that silently disable
  nothing are reported", graduating the cop from `pending` to enabled; mastodon's
  capture is RuboCop 1.91.0, the embedded defaults are 1.82.1 (before that PR).
  discourse is also on RuboCop 1.91.0 but does not hit this disagreement because
  its `.rubocop.yml` chain (`rubocop-discourse`'s `stree-compat.yml`) sets
  `AllCops: DisabledByDefault: true`, so the cop reads `false` on both sides
  instead.

No `crates/config` change was needed for either case: elysium correctly implements
RuboCop 1.82.1's actual default, verified against the real
`rubocop/config/default.yml` at both the `v1.82.1` and `v1.63.4`/`v1.91.0` tags.

## Remaining other-key disagreements

None of the `All-keys agreement` shortfall traces to a `crates/config` bug either;
every sampled case is one of three causes, all confirmed against the real
RuboCop/RuboCop-Rails sources:

- **Cosmetic metadata drift** (`Description`, `StyleGuide`, `Reference`,
  `References`, `VersionAdded` — the large majority of the ~90 non-agreeing keys
  per app): RuboCop rewords cop documentation and style-guide/reference URLs
  between releases without changing behavior. E.g. `Lint/DuplicateBranch`'s
  `Description` in RuboCop 1.91.0 soft-wraps mid-sentence where 1.82.1 does not —
  same text, different line breaks.
- **New parameters added to RuboCop core after 1.82.1** (`EnforcedStyleAlignWith`/
  `SupportedStylesAlignWith` on `Layout/IndentationWidth`, `IndentationWidth` on
  `Layout/ClosingParenthesisIndentation`/`Layout/CommentIndentation`,
  `IgnoreDuplicateElseBranch` on `Lint/DuplicateBranch`, `AllowRBSInlineAnnotation`/
  `AllowSteepAnnotation`/`AllowYARDCommentBlockSeparator` on
  `Layout/LeadingCommentSpace`, and the `infinite?` entry added to
  `Naming/PredicateMethod`/`Style/IfWithBooleanLiteralBranches`/
  `Style/RedundantCondition`'s allowlists per RuboCop's changelog entry
  "[#14823](https://github.com/rubocop/rubocop/issues/14823): Add the built-in
  `infinite?` method to the allowlists for `Naming/PredicateMethod`,
  `Style/IfWithBooleanLiteralBranches`, and `Style/RedundantCondition`"). Verified
  absent from both the real `v1.82.1` and (where applicable) `v1.63.4` tags of
  `rubocop/config/default.yml`, present in `v1.91.0`.
- **RuboCop extension gems (`rubocop-rails`, `rubocop-rspec`) overriding
  non-extension cops.** `rubocop-rails`'s own `config/default.yml` reconfigures
  several RuboCop-core `Lint`/`Style` cops directly:
  `Lint/UselessMethodDefinition` gains
  `Exclude: ['**/app/controllers/**/*.rb', '**/app/mailers/**/*.rb']` ("Avoids
  conflict with `Rails/LexicallyScopedActionFilter` cop", added by rubocop-rails
  [#1500](https://github.com/rubocop/rubocop-rails/pull/1500) "Exclude controllers
  and mailers from `Lint/UselessMethodDefinition`"); `Lint/UselessAccessModifier`
  gains a merged `ContextCreatingMethods` list (rubocop-rails
  [#1385](https://github.com/rubocop/rubocop-rails/pull/1385) "Make
  `Lint/UselessAccessModifier` aware of `ActiveSupport::Concern`..."); and
  `Lint/NumberConversion`, `Lint/RedundantSafeNavigation`, `Lint/SafeNavigationChain`,
  `Style/InvertibleUnlessCondition`, `Style/FormatStringToken`, and
  `Style/SymbolProc` all gain extra `AllowedMethods`/`InverseMethods` entries from
  the same file (confirmed directly against a locally installed
  `rubocop-rails-2.22.1/config/default.yml`). `Metrics/BlockLength` and
  `Style/FrozenStringLiteralComment` similarly pick up extra `Exclude` globs (and,
  for `Metrics/BlockLength`, an `inherit_mode: merge: [Exclude]` key) from
  extension-gem configuration merged ahead of the app's own `.rubocop.yml`.
  Elysium does not embed extension-gem defaults yet (Phase 6), so none of these
  overrides apply; this is the same, already-documented gap the
  `EXTENSION_DEPARTMENTS` skip list exists for, just surfacing on a
  RuboCop-core-named cop instead of a `Rails/*`-named one.

Cops entirely absent from one side (`Discourse/*` app-local cops,
`RSpecRails/*`/`I18n/*` extension-gem cops not in `EXTENSION_DEPARTMENTS`, and
core cops added or removed between RuboCop releases) are counted under "Skew
count" in the table above and are not scored either way.
