{foo: 1, bar: 2, baz: 3}.select { |k, v| k.in?(%I[#{foo} bar]) }
                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `slice(:"#{foo}", :bar)` instead.
