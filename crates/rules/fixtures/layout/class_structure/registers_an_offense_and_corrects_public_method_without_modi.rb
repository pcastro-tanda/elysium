class A
  private def foo
  end

  def bar
  ^^^^^^^ `public_methods` is supposed to appear before `private_methods`.
  end
end
