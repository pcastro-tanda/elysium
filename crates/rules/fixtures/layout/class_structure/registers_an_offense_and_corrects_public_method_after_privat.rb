class A
  def foo
  end
  private :foo

  def bar
  ^^^^^^^ `public_methods` is supposed to appear before `private_methods`.
  end
end
