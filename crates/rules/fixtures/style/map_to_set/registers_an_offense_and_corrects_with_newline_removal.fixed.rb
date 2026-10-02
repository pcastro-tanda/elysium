{foo: bar}
  .to_set { |k, v| [k.to_s, v.do_something] }
  .freeze
