Hash[x.merge(y).map {|k, v| [k, foo(v)]}]
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_values` over `Hash[_.map {...}]`.
