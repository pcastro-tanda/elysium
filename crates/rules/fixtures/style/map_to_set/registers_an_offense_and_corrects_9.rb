foo.collect { [_1.to_s, _2.to_i] }.to_set
    ^^^^^^^ Pass a block to `to_set` instead of calling `collect.to_set`.
