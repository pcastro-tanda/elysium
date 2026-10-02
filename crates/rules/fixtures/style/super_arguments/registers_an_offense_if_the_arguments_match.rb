def test(a, b)
  super(a, b) { x }
  ^^^^^^^^^^^ Call `super` without arguments and parentheses when the signature is identical.
end
