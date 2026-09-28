{a: 1, b: 2}.map {|k, v| [k.to_s, v]}.to_h {|k, v| [v, k]}
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_keys` over `map {...}.to_h`.
