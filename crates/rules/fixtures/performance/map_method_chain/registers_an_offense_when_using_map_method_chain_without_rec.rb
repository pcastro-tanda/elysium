map(&:foo).map(&:bar)
^^^^^^^^^^^^^^^^^^^^^ Use `map { |x| x.foo.bar }` instead of `map` method chain.
