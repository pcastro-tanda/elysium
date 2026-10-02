foo.map { [_1.to_s, _2.to_i] }.to_set
    ^^^ Pass a block to `to_set` instead of calling `map.to_set`.
