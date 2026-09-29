def foo
  def bar.baz(&block)
    qux(&block)
  end
end
