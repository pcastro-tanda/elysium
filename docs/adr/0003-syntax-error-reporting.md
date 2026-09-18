# ADR 0003: Syntax errors mirror `Lint/Syntax`, including deduplication

Status: accepted

## Context

Prism is a recovering parser and frequently emits two diagnostics at one
location, e.g. `unexpected 'end'; expected an expression after the operator`
followed by `unexpected 'end', assuming it is closing the parent method
definition`. RuboCop, using the Prism translation layer, reports only the
first: `Cop::Base#add_offense` ignores a second offense from the same cop on
the same range.

## Decision

- Syntax errors are `Lint/Syntax` diagnostics with severity `fatal`, message
  text taken verbatim from Prism, and Prism's location. With
  `ParserEngine: parser_prism` RuboCop produces identical messages and
  positions (verified on three fixtures, including a multi-byte column).
- A file with syntax errors runs no other rule, as in RuboCop.
- The engine deduplicates `(rule, span)` pairs for every rule, not only
  `Lint/Syntax`; the first-emitted diagnostic wins. This is RuboCop's general
  behaviour and keeps rule code from having to guard against double firing.
- Message text does not include RuboCop's `(Using Ruby X parser; configure
  using TargetRubyVersion...)` suffix. That suffix is `parser`-gem advice and
  has no meaning here. Documented divergence.

## Consequences

- Users on RuboCop's default `parser` engine see different message text and
  sometimes different positions for syntax errors (e.g. `unexpected token
  kEND`). Matching the legacy engine would mean re-implementing another
  parser's error recovery; out of scope by the parser decision.
