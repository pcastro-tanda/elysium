foo.map { [_1, _1 * 2] }.to_h
    ^^^ Pass a block to `to_h` instead of calling `map.to_h`.
