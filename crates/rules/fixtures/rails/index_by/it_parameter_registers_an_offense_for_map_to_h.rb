x.map { [it.to_sym, it] }.to_h
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `index_by` over `map { ... }.to_h`.
