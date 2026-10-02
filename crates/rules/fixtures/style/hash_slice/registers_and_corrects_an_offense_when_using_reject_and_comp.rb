{foo: 1, bar: 2, baz: 3}.reject { |k, v| k != :bar }
                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `slice(:bar)` instead.
