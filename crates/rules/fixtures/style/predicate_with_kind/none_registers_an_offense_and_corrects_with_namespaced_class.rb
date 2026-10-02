array.none? { |x| x.is_a?(ActiveRecord::Base) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `none?(ActiveRecord::Base)` to `none? { ... }` with a kind check.
