foo.map { |x| x * 2 }.to_set.bar
    ^^^ Pass a block to `to_set` instead of calling `map.to_set`.
