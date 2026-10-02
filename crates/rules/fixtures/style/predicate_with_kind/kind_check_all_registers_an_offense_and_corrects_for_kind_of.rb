array.all? { |x| x.kind_of?(String) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `all?(String)` to `all? { ... }` with a kind check.
