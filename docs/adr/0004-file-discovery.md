# ADR 0004: File discovery honours `.gitignore` by default

Status: accepted

## Context

RuboCop's `TargetFinder` walks directories and filters through
`AllCops/Include` and `AllCops/Exclude`; it does not read `.gitignore`.
Projects work around this by mirroring ignore rules into `Exclude`. A native
tool that walks `node_modules`, `tmp`, or build output when the user did not
ask for it feels broken next to ripgrep, Ruff, and Biome.

## Decision

- Directory walks use the `ignore` crate's parallel walker with
  `.gitignore`, global gitignore, and `.git/info/exclude` enabled;
  `--no-gitignore` disables it. Hidden files are not skipped (RuboCop lints
  `.irbrc`, `.pryrc`, `.simplecov`).
- Files are then filtered by RuboCop's default `Include`/`Exclude` patterns,
  relative to the working directory, and the Ruby shebang check for
  extensionless files.
- Explicit file arguments are always linted, matching RuboCop.
- Glob arguments are expanded relative to the working directory and filtered
  the same way as a directory walk.

## Consequences

- A file that is gitignored but listed in `Include` is skipped here and
  linted by RuboCop. This is the one intended divergence; `--no-gitignore`
  restores RuboCop's behaviour exactly.
- Once `.rubocop.yml` loading lands (Phase 2), `AllCops/Include` and
  `Exclude` from config replace the defaults in the same `FileMatcher`.
