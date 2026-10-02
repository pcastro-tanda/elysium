{foo: 1, bar: 2, baz: 3}.select { |k, v| ![1, 2].include?(v) }
