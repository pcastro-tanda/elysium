def test(a, &blk)
  super(a) { x }
  ^^^^^^^^ Call `super` without arguments and parentheses when all positional and keyword arguments are forwarded.
end
