Hash[x.merge(y).map {|k, v| [k.to_sym, v]}]
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_keys` over `Hash[_.map {...}]`.
