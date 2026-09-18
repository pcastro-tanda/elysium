# ADR 0001: One traversal with static dispatch, no node facade

Status: accepted

## Context

Rules need to observe AST nodes. Two settled constraints: all rules run in a
single walk, and dispatch is static (no per-node heap allocation, no callback
registries). Open question was how much of Prism's generated API to wrap.

## Decision

- `ruby_ast` re-exports Prism's 151 node structs unchanged under
  `ruby_ast::node`. They are already zero-cost pointer wrappers with typed
  accessors; a second layer of structs would be 15k lines of glue that adds
  nothing but a place for bugs.
- What `ruby_ast` adds: a `NodeKind` discriminant (`#[repr(u8)]`, generated
  at build time from Prism's `config.json`), byte-offset `Span`s, and an
  enter/leave `Visitor`.
- The child traversal `visit_children` is also generated from `config.json`
  rather than reusing Prism's default `visit_*` functions. Prism's defaults
  call typed `visit_statements_node`-style methods for single-kind fields and
  never route them through `visit`, so `StatementsNode`, `ArgumentsNode`,
  `ParametersNode` and others would be invisible to an enter/leave visitor.
- Rules declare `META.kinds: &'static [NodeKind]`. The `rules` crate
  generates a `Dispatch` impl that matches on `NodeKind` and calls only the
  subscribed rules, so the cost per node is one `match` plus the enabled
  rules for that kind.

## Consequences

- Rule code reads Prism types directly (`node.as_call_node()`), so a Prism
  API change touches rules too, not only `ruby_ast`. Mitigation: the vendored
  Prism is pinned (ADR 0002) and upgraded deliberately.
- The engine computes `kind_of(node)` once per node and passes it to
  `Dispatch`; rules never re-derive it.
