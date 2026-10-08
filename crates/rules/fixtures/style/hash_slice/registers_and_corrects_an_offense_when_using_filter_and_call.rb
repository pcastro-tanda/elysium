{foo: 1, bar: 2, baz: 3}.filter { |k, v| [:foo, :bar].include?(k) }
                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `slice(:foo, :bar)` instead.
