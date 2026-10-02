{foo: bar}
  .map { |k, v| [k.to_s, v.do_something] }
   ^^^ Pass a block to `to_set` instead of calling `map.to_set`.
  .to_set
  .freeze
