array.max { |a, b| a[:foo] <=> b[:foo] }
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `max_by { |a| a[:foo] }` instead of `max { |a, b| a[:foo] <=> b[:foo] }`.
