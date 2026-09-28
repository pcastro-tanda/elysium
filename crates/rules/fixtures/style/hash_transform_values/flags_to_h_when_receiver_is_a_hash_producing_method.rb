x.merge(y).to_h {|k, v| [k, foo(v)]}
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_values` over `to_h {...}`.
