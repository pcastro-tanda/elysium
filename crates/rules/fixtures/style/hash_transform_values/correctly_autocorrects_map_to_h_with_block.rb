{a: 1, b: 2}.map {|k, v| [k, foo(v)]}.to_h {|k, v| [v, k]}
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_values` over `map {...}.to_h`.
