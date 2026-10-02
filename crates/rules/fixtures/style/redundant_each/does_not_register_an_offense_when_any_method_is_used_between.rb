string.each_char.map(&:to_i).reverse.each_with_index.map { |v, i| do_something(v, i) }
