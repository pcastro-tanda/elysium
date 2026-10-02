array.any? { it.kind_of?(String) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `any?(String)` to `any? { ... }` with a kind check.
