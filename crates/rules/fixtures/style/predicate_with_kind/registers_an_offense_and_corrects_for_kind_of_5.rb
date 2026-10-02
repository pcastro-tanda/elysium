array.any? { _1.kind_of?(String) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `any?(String)` to `any? { ... }` with a kind check.
