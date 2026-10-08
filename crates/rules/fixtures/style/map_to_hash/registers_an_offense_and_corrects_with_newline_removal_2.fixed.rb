{foo: bar}
  .to_h { |k, v| [k.to_s, v.do_something] }
  .freeze
