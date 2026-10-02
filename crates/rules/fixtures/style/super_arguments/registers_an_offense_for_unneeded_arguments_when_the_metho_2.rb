def foo(a)
  super(a).foo { _1 }
  ^^^^^^^^ Call `super` without arguments and parentheses when the signature is identical.
end
