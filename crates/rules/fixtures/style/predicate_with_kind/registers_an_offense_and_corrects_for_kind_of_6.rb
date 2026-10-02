array.all? { _1.kind_of?(String) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `all?(String)` to `all? { ... }` with a kind check.
