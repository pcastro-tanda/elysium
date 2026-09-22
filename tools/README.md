# tools/

Development tooling only. Nothing here is a runtime dependency of the
`linter`/`rules` crates, is loaded by the compiled `elysium` binary, or ships
in a release artifact.

## port_spec.rb

Ports a RuboCop cop's RSpec examples into elysium's fixture format (phase3
contract, section 3) by actually running the real spec against the real cop
from a RuboCop source checkout, with `expect_offense` / `expect_correction` /
`expect_no_offenses` / `expect_no_corrections` monkeypatched to record their
already-interpolated arguments before delegating to RuboCop's own
implementation. Every ported fixture is therefore taken verbatim from a
passing assertion against the real cop, not a manual transcription.

### Usage

```sh
ruby tools/port_spec.rb \
  --cop Style/TrailingCommaInArguments \
  --rubocop-src /Users/paulo/Work/lab/corpus/rubocop-1.82.1 \
  --out crates/rules/fixtures
```

`--rubocop-src` is any full RuboCop source checkout (has both `lib/` and
`spec/`; the installed gem alone does not ship `spec/`). `--out` defaults to
`crates/rules/fixtures`; the tool derives `<dept>/<snake>` from the spec file
that actually declares `RSpec.describe RuboCop::Cop::<Dept>::<Name>` (so it
always matches RuboCop's own file layout, including acronym-heavy cop names)
and writes into `<out>/<dept>/<snake>/`.

### What it produces, per example

- `<case>.rb` — the exact `expect_offense` annotated source (or the plain
  `expect_no_offenses` source when there's no offense), byte for byte.
- `<case>.fixed.rb` — the exact `expect_correction` argument, when present.
- `<case>.nofix` — empty marker, written when the example calls
  `expect_no_corrections`.
- `<case>.singlepass` — empty marker, written when the example calls
  `expect_correction(..., loop: false)`; RulesFoundation's harness must use a
  single `apply_fixes` pass (not the convergence loop) for these cases.
- `<case>.yml` — full `.rubocop.yml` shape, written only when the example's
  effective settings differ from real defaults:
  - `cop_config` entries whose value actually differs from
    `RuboCop::ConfigLoader.default_configuration.for_cop(cop_class)` (a spec's
    `let(:cop_config)` can restate a default value verbatim — that must not
    produce a fixture `.yml`);
  - `other_cops` entries, verbatim;
  - `AllCops: {TargetRubyVersion: X}` when the example's `ruby_version` is
    explicitly set below the Prism-mode default (see below);
  - a `# file: <path>` leading comment when the example passed a file name to
    `expect_offense`/`expect_no_offenses`.

### Case naming

Deterministic snake_case built from every `context`/`describe` description on
the path to the example plus the `it` description, joined and slugified. When
the joined name would exceed 60 characters, context tokens are dropped from
the front (keeping the most specific, rightmost — usually the `it`
description — intact) until it fits; final uniqueness is guaranteed against
every name already assigned in the run (not just same-prefix collisions), so
truncation can never silently overwrite an unrelated case's fixture.

### Prism-only filtering

The harness runs with `PARSER_ENGINE=parser_prism`, which is exactly the knob
RuboCop's own `rubocop/rspec/cop_helper.rb` and CI use to switch the spec
suite's default `ruby_version`/parser to Prism-compatible settings (3.3+), and
the same `broken_on: :prism` / `unsupported_on: :prism` RSpec exclusion tags
RuboCop's own `spec_helper.rb` applies for that mode are applied here too.
Examples that still explicitly pin a `ruby_version` below 3.3 (untagged) are
additionally skipped and reported by name — elysium's Prism-only engine has
no way to parse what they exercise. Examples that never reach
`expect_offense`/`expect_no_offenses` (direct `RuboCop::Cop::...` internals
access, or a raised error) are reported as "not captured" with the exception
message when there is one, instead of silently producing an incomplete
fixture set.

### Requirements

The exact RuboCop version pinned by `--rubocop-src`'s
`lib/rubocop/version.rb`, and `rspec`, already installed as system gems
(`gem list rubocop rspec`). The tool writes a scratch `Gemfile` under a temp
directory and resolves it with `bundle lock --local` — no network access, and
`--rubocop-src` itself is never modified.

### Regenerating a directory

The command at the top of this section regenerates
`crates/rules/fixtures/style/trailing_comma_in_arguments/` with identical
per-case content (verified by comparing the sorted multiset of file content
hashes between a from-scratch run and the committed directory — 374 files,
zero diff). Filenames may differ from a hand-ported directory predating this
tool, since case naming here is fully generic (no per-cop knowledge); content
per case does not.
