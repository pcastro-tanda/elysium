relation&.select(:column_name)&.map(&:column_name)
          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `pluck(:column_name)` instead of `select` with `map`.
