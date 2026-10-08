foo.collect { [_1, _1 * 2] }.to_h
    ^^^^^^^ Pass a block to `to_h` instead of calling `collect.to_h`.
