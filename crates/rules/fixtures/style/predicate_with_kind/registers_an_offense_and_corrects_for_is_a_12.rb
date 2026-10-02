array&.one? { |x| x.is_a?(Integer) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `one?(Integer)` to `one? { ... }` with a kind check.
