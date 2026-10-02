foo.collect { |x| x * 2 }.to_set.bar
    ^^^^^^^ Pass a block to `to_set` instead of calling `collect.to_set`.
