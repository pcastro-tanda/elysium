def foo(a)
  super(a).foo
  ^^^^^^^^ Call `super` without arguments and parentheses when the signature is identical.
end
