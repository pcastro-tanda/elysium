foo.collect { |x| x * 2 }.to_h.bar
    ^^^^^^^ Pass a block to `to_h` instead of calling `collect.to_h`.
