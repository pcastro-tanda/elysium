def method(a = 1, b: 2)
  super(a, b: b)
  ^^^^^^^^^^^^^^ Call `super` without arguments and parentheses when the signature is identical.
end
