array.map(&:foo).collect(&:bar)
      ^^^^^^^^^^^^^^^^^^^^^^^^^ Use `map { |x| x.foo.bar }` instead of `map` method chain.
