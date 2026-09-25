# ADR 0007: Local-variable semantics are built by a second, lazy traversal

Status: accepted

## Context

A handful of cops -- `Lint/UselessAssignment`,
`Lint/ShadowingOuterLocalVariable`, and more to come -- need what RuboCop
calls `VariableForce`: lexical scopes, every local variable's declaration,
assignments and references, block capture, and the branch model that decides
whether an assignment can still be read. That model cannot be produced from
the enter/leave events of ADR 0001's single traversal: it needs child order
the walk does not offer (a post-condition loop's body before its predicate,
`for`'s collection before its index, multiple assignment's value before its
targets, a class's superclass evaluated in the enclosing scope), and it needs
whole-subtree lookahead (`mark_assignments_as_referenced_in_loop` rescans a
loop's descendants once the loop is closed). Interleaving that with rule
dispatch would either reorder the events every existing rule depends on or
push a second, differently-ordered traversal into the engine for every file.

## Decision

- `crates/ruby_semantic` owns its own recursive traversal with explicit child
  ordering, and exposes the finished arena as `Semantics::build(&root)`.
- `linter::Context` holds it in a `OnceCell<Semantics<'a>>` behind
  `Context::semantics()`, built on first call from `Parsed::root()`, exactly
  like `Context::comments()`.
- This is the one sanctioned exception to ADR 0001's "one traversal per
  file". The engine itself never touches the cell: only a rule asking for
  `ctx.semantics()` triggers the walk.

## Consequences

- Files linted without a semantic cop enabled pay nothing: the cell is never
  initialised, so no extra walk, no allocation.
- Files that do use one pay a second walk of the tree plus the arena. The
  walk is a small multiple of the parse cost, and it happens once per file no
  matter how many semantic cops are enabled, because they share the cell.
- `Semantics` borrows the tree, so it is per-file state, not shared: it dies
  with the `Context` and needs no synchronisation across the rayon pool.
- Records are id-indexed into flat `Vec`s and ancestor chains live in one
  shared pool, so building the model is a handful of allocations rather than
  one per node.
- Because the semantic walk is separate, its ordering rules can be checked
  directly against RuboCop's `variable_force` specs
  (`cargo test -p ruby_semantic`) without a rule or a fixture in the way.
