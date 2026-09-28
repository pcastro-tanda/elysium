{a: 1, b: 2}.map {|k, v| [k.to_sym, v]}.to_h
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_keys` over `map {...}.to_h`.
