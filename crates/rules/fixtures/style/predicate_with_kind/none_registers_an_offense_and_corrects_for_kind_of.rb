array.none? { |x| x.kind_of?(String) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `none?(String)` to `none? { ... }` with a kind check.
