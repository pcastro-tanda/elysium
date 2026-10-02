class A
  private def foo; end

  def bar; end
  ^^^^^^^^^^^^ `public_methods` is supposed to appear before `private_methods`.

  private def baz; end

  def qux; end
  ^^^^^^^^^^^^ `public_methods` is supposed to appear before `private_methods`.
end
