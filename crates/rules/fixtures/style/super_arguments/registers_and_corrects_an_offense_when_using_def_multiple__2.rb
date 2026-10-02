def method(a, b, c = 1)
  super(a, b, c)
  ^^^^^^^^^^^^^^ Call `super` without arguments and parentheses when the signature is identical.
end
