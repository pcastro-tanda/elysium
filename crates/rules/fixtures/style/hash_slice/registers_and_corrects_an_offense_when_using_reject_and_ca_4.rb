{foo: 1, bar: 2, baz: 3}.reject { |k, v| %i[foo bar].exclude?(k) }
                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `slice(:foo, :bar)` instead.
