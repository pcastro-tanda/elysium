x.map { [_1.to_sym, _1] }.to_h
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `index_by` over `map { ... }.to_h`.
