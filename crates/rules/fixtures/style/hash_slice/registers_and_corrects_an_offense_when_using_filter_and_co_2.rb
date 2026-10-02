{foo: 1, bar: 2, baz: 3}.filter { |k, v| k == :bar }
                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `slice(:bar)` instead.
