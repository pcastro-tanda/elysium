array.all? { it.kind_of?(String) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `all?(String)` to `all? { ... }` with a kind check.
