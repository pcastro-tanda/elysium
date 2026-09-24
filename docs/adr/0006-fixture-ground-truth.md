# ADR 0006: Fixtures are generated from RuboCop's specs, never hand-edited

Status: accepted

## Context

Every one of the 50 rules' fixtures under `crates/rules/fixtures/` need a
ground truth that is both trustworthy (matches what real RuboCop actually
does, not what a rule's author assumed it does) and cheap to keep in sync as
RuboCop itself evolves. Hand-writing `expect_offense`-equivalent cases invites
two failure modes: a case that quietly encodes the port's own bug instead of
RuboCop's behaviour, and drift as new edge cases in upstream RuboCop specs
never make it into the port.

## Decision

- Fixtures are mechanically extracted from RuboCop's own cop specs by
  `tools/port_spec.rb`, which instruments `expect_offense`/
  `expect_no_offenses`/`expect_correction` inside a real `rspec` run against
  a RuboCop source checkout and serializes each example into a
  `<case>.rb`/`<case>.yml`/`<case>.fixed.rb`/`<case>.offenses` fixture
  directory (see the tool's own header comment and
  `crates/rules/fixtures/README.md`). Fixtures are never hand-edited after
  generation; a wrong or missing case is fixed by changing the extraction
  logic and regenerating, not by patching a `.rb`/`.yml` file by hand.
- A regenerated directory is diffed against the previous one before being
  committed, so a change in extracted cases is always visible in review.
- Classes of RSpec example that cannot be mechanically translated into a
  case a real CLI invocation could ever produce are deleted after
  regeneration and listed, with the specific reason, in
  `crates/rules/fixtures/README.md`:
  - Cases that depend on Ruby process state elysium does not model, e.g. a
    spec that mutates `Encoding.default_external` for the duration of one
    example (`Style/WordArray`'s unicode-word-character case).
  - Cases built on a bare `RuboCop::Config.new` without RuboCop's real
    `config/default.yml` merged in, so an `EnforcedStyle`-like option
    resolves to `nil` instead of its documented default and the spec's
    assertion (e.g. that autocorrection is unconditionally allowed) only
    holds under that artificial nil, never under real CLI config loading
    (`Style/EmptyElse`'s `MissingElse`-disabled autocorrection cases).
  - Cases that construct one cop instance and feed it two separately-parsed
    source files to assert cross-file state (`Lint/DuplicateMethods`'
    same-cop-instance-across-files cases): RuboCop's own CLI, and elysium,
    instantiate a fresh cop per file, so the behaviour the spec exercises
    never occurs in practice.
  - Cases whose assertion is an artifact of RSpec's own example order or
    process working directory rather than the cop's logic (`Lint/
    DuplicateMethods`' absolute-vs-relative-path message cases, which assert
    different text for the same path depending on `Dir.pwd`).
- Where a deleted class's underlying gap is real (not merely un-portable),
  the gap is called out separately in the fixture's rule's `blind_spots`
  metadata (e.g. `Lint/RedundantCopDisableDirective`'s single-cop-removal
  free-text preservation gap is a tracked limitation, not just a deleted
  RSpec artifact) rather than silently dropped.
- When a deleted spec's assertion and elysium's own CLI-driven behaviour
  genuinely disagree about what "correct" means under merged defaults (not
  about portability), the CLI behaviour wins: the spec was written against
  an artificially bare config, and elysium's fixtures and rules must match
  what a user actually gets from `elysium check`/`elysium fix` against a
  real, defaulted configuration, not what an isolated unit test asserted
  under a config nothing real ever produces.

## Consequences

- Fixture correctness is bounded by RuboCop's own spec coverage: a
  behaviour RuboCop's specs never exercise has no fixture either, tracked
  instead through conformance runs against real corpora
  (`docs/conformance/rules.md`) rather than synthetic cases.
- Regenerating a cop's fixtures after a rule change is a single
  `tools/port_spec.rb` invocation, not a manual editing pass, so fixtures
  stay traceable to a specific RuboCop version's spec suite.
- The deleted-case list in `crates/rules/fixtures/README.md` is the
  authoritative record of every place elysium's ground truth intentionally
  diverges from a literal RSpec example; a reviewer who wants to know why a
  case is missing checks there before assuming it was overlooked.