array.all? { _1.is_a?(Integer) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `all?(Integer)` to `all? { ... }` with a kind check.
