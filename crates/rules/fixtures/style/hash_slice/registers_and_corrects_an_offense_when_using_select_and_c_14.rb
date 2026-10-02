{foo: 1, bar: 2, baz: 3}.select { |k, v| !array.exclude?(k) }
                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `slice(*array)` instead.
