Hash[{a: 1, b: 2}.map {|k, v| [k, foo(v)]}]
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_values` over `Hash[_.map {...}]`.
