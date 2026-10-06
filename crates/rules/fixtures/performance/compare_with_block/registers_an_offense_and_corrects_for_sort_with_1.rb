array.sort { |a, b| a[1] <=> b[1] }
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `sort_by { |a| a[1] }` instead of `sort { |a, b| a[1] <=> b[1] }`.
