{foo: 1, bar: 2, baz: 3}.select { |k, v| :bar == k }
                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `slice(:bar)` instead.
