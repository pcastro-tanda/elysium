# ADR 0005: Resolving `inherit_gem` and ERB without a Ruby runtime

Status: accepted

## Context

RuboCop's `ConfigLoader` resolves two constructs that assume a live Ruby
process: `inherit_gem`, which asks Bundler (`Bundler.load.specs`) and then
RubyGems (`Gem::Specification.find_by_name`) for an installed gem's
directory, and ERB embedded in `.rubocop.yml`, which it evaluates before
parsing the result as YAML. elysium has no Ruby runtime and is not going to
embed one; both constructs still had to be handled well enough for real
`.rubocop.yml` files (`inherit_gem: { rubocop-discourse: ... }`,
`inherit_gem: { rubocop-rails-omakase: ... }`) to load.

## Decision

- `inherit_gem` gem directories are found by filesystem search
  (`crates/config/src/gems.rs`), not by asking Bundler or RubyGems: the gem's
  version is read from the nearest `Gemfile.lock`/`gems.locked`, and that
  `<gem>-<version>/` directory is looked up under the usual install roots
  (project-local `vendor/bundle`, `Bundler.bundle_path`-style layout guesses,
  `GEM_HOME`, `GEM_PATH`, and the per-user RubyGems default), plus any
  caller-supplied `extra_roots` for tests. No lockfile entry means no known
  version, so `inherit_gem` on an unlocked gem fails with a clear error
  rather than silently guessing.
- ERB in `.rubocop.yml` is rejected outright (`ConfigError::ErbUnsupported`)
  instead of evaluated. There is no Ruby runtime to run it against, and a
  partial/regex-based ERB evaluator would silently produce wrong config for
  any template that isn't a trivial `<%= ENV['X'] %>` substitution, which is
  worse than a clear error naming the offending file.
- Remote `inherit_from: https://...` is likewise rejected
  (`ConfigError::RemoteUnsupported`) rather than fetched, for the same
  "clear error beats silent wrongness" reasoning, and because fetching
  arbitrary URLs during config load is not a property a linter should have
  by default.

## Consequences

- `inherit_gem` works for the common case — a locked gem installed in a
  reachable root — which covers `rubocop-discourse`, `rubocop-shopify`,
  `rubocop-rails-omakase`, and similar community configs without needing
  Bundler or RubyGems on `PATH`. It does not cover gems installed in a
  location the search roots don't know about (e.g. an unusual `BUNDLE_PATH`
  elysium hasn't seen); those fail loudly instead of silently omitting the
  inherited config.
- ERB rejection is a known, real gap: GitLab's `.rubocop.yml` uses ERB and
  currently fails to load under `elysium check`/`elysium config`.
  `--no-config` (skip `.rubocop.yml` entirely, use embedded defaults) is the
  workaround today. If more real-world configs turn out to need it, the
  next step is not a general ERB implementation but a tiny subset evaluator
  covering only `<%= ENV[...] %>`-style substitutions actually observed in
  the wild, added when a concrete config demands it rather than
  speculatively.
- Remote `inherit_from` rejection has not blocked any of the three
  conformance apps (discourse, forem, mastodon) or GitLab; no app inspected
  so far uses it. Revisit if one does.
</content>
<parameter name="i">Write ADR 0005