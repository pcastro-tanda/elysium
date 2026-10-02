foo.collect { [_1.to_s, _2.to_i] }.to_h
    ^^^^^^^ Pass a block to `to_h` instead of calling `collect.to_h`.
