def foo(a)
  super(a).foo { x }
  ^^^^^^^^ Call `super` without arguments and parentheses when the signature is identical.
end
