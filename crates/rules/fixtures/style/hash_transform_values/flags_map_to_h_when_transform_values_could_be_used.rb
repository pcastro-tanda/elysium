x.map {|k, v| [k, foo(v)]}.to_h
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_values` over `map {...}.to_h`.
