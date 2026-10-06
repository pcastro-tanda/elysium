array&.map(&:foo).map(&:bar).map(&:baz)
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `map { |x| x.foo.bar.baz }` instead of `map` method chain.
