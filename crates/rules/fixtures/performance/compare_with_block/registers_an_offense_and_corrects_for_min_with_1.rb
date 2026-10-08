array.min { |a, b| a[1] <=> b[1] }
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `min_by { |a| a[1] }` instead of `min { |a, b| a[1] <=> b[1] }`.
