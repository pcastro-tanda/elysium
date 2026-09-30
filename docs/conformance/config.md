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
| discourse | 1.91.0 | 644 | 644/644 (100.0%) | 644/644 (100.0%) | 0 (+279 extension-department) |
| mastodon | 1.91.0 | 628 | 628/628 (100.0%) | 619/628 (98.6%) | 0 (+320 extension-department) |
| forem | 1.63.4 | 549 | 548/549 (99.8%) | 471/549 (85.8%) | 48 (+311 extension-department) |

"Cops compared" is the agreement denominator: cops present on both sides, outside
a skipped extension department. `Enabled agreement` is the metric
`conformance-config` gates on by default; `All-keys agreement` additionally
requires every other resolved key (`Include`, `Exclude`, `AllowedMethods`, etc.) to
match and is reported for completeness only.

> The discourse and mastodon rows were measured against the embedded RuboCop
> 1.91.0 defaults. The forem row is the last measurement taken against the
> embedded 1.82.1 defaults: forem's checkout is no longer on this machine
> (`/tmp/corpus/forem` and `/Users/paulo/Work/lab/corpus/forem` are both gone),
> only its `forem.show-cops.yml` capture, so it could not be re-run. Its numbers
> are a floor, not a current reading — see the version-skew note below for why
> most of its shortfall is expected to survive the 1.91.0 swap.

## Version skew is now zero for the 1.91.0-pinned apps

The embedded defaults are RuboCop 1.91.0, which is exactly the release discourse
and mastodon pin, so for those two apps there is no cop-set skew and no `Enabled`
disagreement left at all. Both were measured before the swap at 593/644 and
570/628 on all keys; almost all of that gap was `Preview` sections (RuboCop 1.91.0
gives 50 cops and `AllCops` a `Preview` block holding the defaults expected in
the next major release, and drops the block from the resolved configuration) plus cosmetic
`Description`/`VersionAdded` drift, and both are gone.

**forem stays skewed on purpose.** Its `Gemfile.lock` pins RuboCop 1.63.4, i.e.
27 minor releases behind the embedded defaults, so cops added after 1.63.4 are
absent from its capture, cops removed since are absent from ours, and defaults
that changed in between disagree. Two concrete, verified cases:

- **`Lint/ShadowingOuterLocalVariable`** (elysium `Enabled: false`, forem's truth
  `Enabled: true`). The cop's default flipped from `true` to `false` at RuboCop
  1.76 (`VersionChanged: '1.76'` in the embedded `config/default.yml`); 1.63.4
  predates that and reports no `VersionChanged` for the cop at all.
- **`Style/DoubleCopDisableDirective`** exists in forem's 1.63.4 capture and not
  in the embedded 1.91.0 defaults: RuboCop removed it in favour of
  `Lint/CopDirectiveSyntax`. The embedded `config/obsoletion.yml` lists it under
  `removed` with `severity: warning`, so a configuration still naming it warns
  rather than failing.

`Lint/CopDirectiveSyntax` is no longer a disagreement anywhere: RuboCop 1.91.0
enables it by default ([#15616](https://github.com/rubocop/rubocop/pull/15616)),
and the embedded defaults now say so.

## Remaining other-key disagreements

discourse has none. mastodon's nine are all the same cause, already documented:

- **`rubocop-rails` overriding non-extension cops.** All nine cops appear in
  `rubocop-rails`'s own `config/default.yml` (verified against a locally
  installed `rubocop-rails-2.37.0`; mastodon's `Gemfile.lock` pins 2.38.0),
  which reconfigures RuboCop-core `Lint`/`Style` cops directly. Elysium does not
  embed extension-gem defaults yet (Phase 6), so none of those overrides apply:
  - `Lint/UselessMethodDefinition` gains
    `Exclude: ['**/app/controllers/**/*.rb', '**/app/mailers/**/*.rb']`
    (rubocop-rails [#1500](https://github.com/rubocop/rubocop-rails/pull/1500),
    "Exclude controllers and mailers from `Lint/UselessMethodDefinition`");
  - `Lint/UselessAccessModifier` gains a merged `ContextCreatingMethods` list
    (`class_methods`, `included`, `prepended`, `concern`, `concerning`) and the
    `inherit_mode: merge: [ContextCreatingMethods]` key that merges it
    (rubocop-rails [#1385](https://github.com/rubocop/rubocop-rails/pull/1385),
    "Make `Lint/UselessAccessModifier` aware of `ActiveSupport::Concern`...");
  - `Lint/NumberConversion`, `Lint/RedundantSafeNavigation`,
    `Lint/SafeNavigationChain`, `Style/CollectionCompact`,
    `Style/FormatStringToken`, `Style/InvertibleUnlessCondition` and
    `Style/SymbolProc` all gain Rails-specific `AllowedMethods`/
    `AllowedReceivers`/`InverseMethods` entries from the same file (`ago`,
    `from_now`, `seconds`, `params`, `present?`/`blank?`, `exclude?`, ...).

  This is the same gap the `EXTENSION_DEPARTMENTS` skip list exists for, just
  surfacing on a RuboCop-core-named cop instead of a `Rails/*`-named one.

forem's larger shortfall additionally traces to **cosmetic metadata drift**
(`Description`, `StyleGuide`, `Reference`, `References`, `VersionAdded`: RuboCop
rewords cop documentation and soft-wraps it differently between releases) and to
**core parameters added between 1.63.4 and 1.91.0**, neither of which is a
`crates/config` bug.

Cops entirely absent from one side (`Discourse/*` app-local cops,
`RSpecRails/*`/`I18n/*` extension-gem cops not in `EXTENSION_DEPARTMENTS`, and
core cops added or removed between RuboCop releases) are counted under "Skew
count" in the table above and are not scored either way.
