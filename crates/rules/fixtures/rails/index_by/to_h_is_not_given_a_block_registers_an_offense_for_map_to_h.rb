x.map { |el| [el.to_sym, el] }.to_h
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `index_by` over `map { ... }.to_h`.
