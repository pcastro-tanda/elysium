select(:column_name).where(conditions).map(&:column_name)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `pluck(:column_name)` instead of `select` with `map`.
