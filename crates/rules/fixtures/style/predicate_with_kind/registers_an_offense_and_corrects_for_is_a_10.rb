array&.all? { |x| x.is_a?(Integer) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `all?(Integer)` to `all? { ... }` with a kind check.
