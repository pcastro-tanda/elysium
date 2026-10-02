array.any? { |x| x.instance_of?(Float) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `any?(Float)` to `any? { ... }` with a kind check.
