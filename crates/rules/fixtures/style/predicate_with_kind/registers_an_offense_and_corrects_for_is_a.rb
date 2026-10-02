array.any? { it.is_a?(Integer) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `any?(Integer)` to `any? { ... }` with a kind check.
