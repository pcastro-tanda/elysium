array.collect(&:foo).collect(&:bar)
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `collect { |x| x.foo.bar }` instead of `collect` method chain.
