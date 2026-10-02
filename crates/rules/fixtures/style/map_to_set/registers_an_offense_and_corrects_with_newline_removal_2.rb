{foo: bar}
  .collect { |k, v| [k.to_s, v.do_something] }
   ^^^^^^^ Pass a block to `to_set` instead of calling `collect.to_set`.
  .to_set
  .freeze
