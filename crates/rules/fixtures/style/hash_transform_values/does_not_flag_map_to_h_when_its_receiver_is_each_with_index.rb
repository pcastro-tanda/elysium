[1, 2, 3].each_with_index.map { |k, v| [k, foo(v)] }.to_h
