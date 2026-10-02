{foo: 1, bar: 2, baz: 3}.select { |k, v| !%W[#{foo} bar].exclude?(k) }
                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `slice("#{foo}", 'bar')` instead.
