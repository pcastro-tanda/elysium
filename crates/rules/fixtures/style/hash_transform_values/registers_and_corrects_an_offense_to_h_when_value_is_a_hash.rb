{a: 1, b: 2}.to_h { |key, val| [key, { value: val }] }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_values` over `to_h {...}`.
