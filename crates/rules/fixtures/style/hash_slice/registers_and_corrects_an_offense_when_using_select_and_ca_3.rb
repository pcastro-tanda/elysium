{foo: 1, bar: 2, baz: 3}.select { |k, v| %I[#{foo} bar].include?(k) }
                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `slice(:"#{foo}", :bar)` instead.
