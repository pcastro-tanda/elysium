array.max { |a, b| a[1] <=> b[1] }
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `max_by { |a| a[1] }` instead of `max { |a, b| a[1] <=> b[1] }`.
