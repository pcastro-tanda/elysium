# ADR 0002: Vendor `ruby-prism` with an additive `parse_with_options`

Status: accepted

## Context

The published `ruby-prism` 1.9.0 crate exposes only `parse(&[u8])`, which
uses Prism's default options. RuboCop drives Prism through
`Prism::Translation::Parser` with `partial_script: true`, so top-level
`yield`/`return` in templates (`app/views/**/*.builder`, `.jbuilder`) are not
syntax errors. Without that option we reported a false `Lint/Syntax` on
GitLab's `xml.atom.builder` where RuboCop reports nothing. Future needs on
the same struct: `version` (for `TargetRubyVersion`), `encoding_locked`,
`frozen_string_literal`.

## Decision

- `vendor/ruby-prism/` is a copy of the 1.9.0 crate source, wired in with
  `[patch.crates-io]`. `ruby-prism-sys` (the C library) stays on crates.io.
- One additive change: `parse_with_options(source, &ParseOptions)` and the
  `SyntaxVersion` enum. `parse` is untouched. The change is contained in a
  single `unsafe` block that zero-initialises `pm_options_t`, which Prism
  documents as the unset state for every field.
- `ruby_ast`'s build script reads the node schema from
  `vendor/ruby-prism/vendor/prism-1.9.0/config.json`, so `NodeKind` and the
  parser can never disagree on the node list.
- `ruby_ast::ParseOptions::default()` is `partial_script: true`, matching
  RuboCop.
- Second additive change (Phase 4): `build.rs` emits `#[derive(Clone, Copy)]`
  on `Node` and every generated node struct. Each is a `(parser, pointer,
  PhantomData)` triple that Prism's own `as_node()`/`as_*_node()` accessors
  already duplicate freely; the derive only lets `ruby_semantic` store nodes
  in its arenas without round-tripping through those accessors.

## Consequences

- Upgrading Prism means re-copying the crate and re-applying one function
  and two `build.rs` lines. Offer the function upstream so the vendored copy
  can eventually go away.
- `unsafe_code = "deny"` remains for every workspace crate; the vendored
  crate is the only place `unsafe` exists.
