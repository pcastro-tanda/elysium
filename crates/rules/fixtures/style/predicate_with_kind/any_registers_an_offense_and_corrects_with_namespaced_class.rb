array.any? { |x| x.is_a?(ActiveRecord::Base) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `any?(ActiveRecord::Base)` to `any? { ... }` with a kind check.
