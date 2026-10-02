foo.collect(&:do_something).to_set
    ^^^^^^^ Pass a block to `to_set` instead of calling `collect.to_set`.
