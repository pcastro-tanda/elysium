array.min { |a, b| a['foo'] <=> b['foo'] }
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `min_by { |a| a['foo'] }` instead of `min { |a, b| a['foo'] <=> b['foo'] }`.
