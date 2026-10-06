array.sort { |a, b| a[:foo] <=> b[:foo] }
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `sort_by { |a| a[:foo] }` instead of `sort { |a, b| a[:foo] <=> b[:foo] }`.
