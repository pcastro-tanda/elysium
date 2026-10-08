array.one? { |x| x.is_a?(ActiveRecord::Base) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `one?(ActiveRecord::Base)` to `one? { ... }` with a kind check.
