def test(a, &blk)
  super(a) { _1 }
  ^^^^^^^^ Call `super` without arguments and parentheses when all positional and keyword arguments are forwarded.
end
