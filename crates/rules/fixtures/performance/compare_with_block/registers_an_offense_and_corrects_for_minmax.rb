array.minmax { |a, b| a.foo <=> b.foo }
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `minmax_by(&:foo)` instead of `minmax { |a, b| a.foo <=> b.foo }`.
