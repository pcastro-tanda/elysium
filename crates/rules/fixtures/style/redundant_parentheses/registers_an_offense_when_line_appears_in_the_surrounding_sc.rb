class A
  def same
    (foo)
    ^^^^^ Don't use parentheses around a method call.
  end
  LINE = __LINE__
end
