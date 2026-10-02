x.map { |el| [el.to_sym, el] }.to_h { |k, v| [v, k] }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `index_by` over `map { ... }.to_h`.
