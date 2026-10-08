{foo: 1, bar: 2, baz: 3}.select { |k, v| k.in?(%i[foo bar]) }
                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `slice(:foo, :bar)` instead.
