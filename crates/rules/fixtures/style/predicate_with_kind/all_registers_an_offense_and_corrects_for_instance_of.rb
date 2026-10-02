array.all? { |x| x.instance_of?(Float) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `all?(Float)` to `all? { ... }` with a kind check.
