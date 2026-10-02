foo.collect { [_1, _1 * 2] }.to_set
    ^^^^^^^ Pass a block to `to_set` instead of calling `collect.to_set`.
