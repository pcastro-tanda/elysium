array.all? { |x| x.is_a?(ActiveRecord::Base) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `all?(ActiveRecord::Base)` to `all? { ... }` with a kind check.
