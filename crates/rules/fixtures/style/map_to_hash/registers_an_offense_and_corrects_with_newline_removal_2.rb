{foo: bar}
  .collect { |k, v| [k.to_s, v.do_something] }
   ^^^^^^^ Pass a block to `to_h` instead of calling `collect.to_h`.
  .to_h
  .freeze
