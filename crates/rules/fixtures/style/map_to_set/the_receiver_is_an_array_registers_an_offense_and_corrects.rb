[1, 2, 3].map { |x| [x, x * 2] }.to_set
          ^^^ Pass a block to `to_set` instead of calling `map.to_set`.
