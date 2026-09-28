[1, 2, 3].each.with_index.map { |k, v| [k, foo(v)] }.to_h
