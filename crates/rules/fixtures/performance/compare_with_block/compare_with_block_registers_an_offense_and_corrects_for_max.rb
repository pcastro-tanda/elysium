array.max { |a, b| a.foo <=> b.foo }
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `max_by(&:foo)` instead of `max { |a, b| a.foo <=> b.foo }`.
