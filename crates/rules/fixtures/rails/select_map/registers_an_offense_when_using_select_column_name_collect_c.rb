Model.select(:column_name).collect(&:column_name)
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `pluck(:column_name)` instead of `select` with `collect`.
