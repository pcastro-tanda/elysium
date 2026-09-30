class A
  ROOT = __FILE__
  def same
    (foo)
    ^^^^^ Don't use parentheses around a method call.
  end
end
