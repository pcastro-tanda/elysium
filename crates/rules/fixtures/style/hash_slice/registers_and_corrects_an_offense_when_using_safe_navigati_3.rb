{foo: 1, bar: 2, baz: 3}&.select { |k, v| !%i[foo bar].exclude?(k) }
                          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `slice(:foo, :bar)` instead.
