def foo(a)
  def bar(b:)
    super(b: b)
    ^^^^^^^^^^^ Call `super` without arguments and parentheses when the signature is identical.
  end
  super(a)
  ^^^^^^^^ Call `super` without arguments and parentheses when the signature is identical.
end
