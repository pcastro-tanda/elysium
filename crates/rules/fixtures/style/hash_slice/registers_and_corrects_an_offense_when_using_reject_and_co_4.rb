{foo: 1, bar: 2, baz: 3}.reject { |k, v| :bar != k }
                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `slice(:bar)` instead.
