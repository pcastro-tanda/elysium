x.merge(y).to_h {|k, v| [k.to_sym, v]}
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_keys` over `to_h {...}`.
