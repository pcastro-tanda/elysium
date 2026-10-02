foo.map { |x| x * 2 }.to_h.bar
    ^^^ Pass a block to `to_h` instead of calling `map.to_h`.
