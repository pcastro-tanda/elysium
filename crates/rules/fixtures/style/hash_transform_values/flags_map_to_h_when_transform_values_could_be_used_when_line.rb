{a: 1, b: 2}.map {|k, v| [k, foo(v)]}.
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_values` over `map {...}.to_h`.
  to_h
