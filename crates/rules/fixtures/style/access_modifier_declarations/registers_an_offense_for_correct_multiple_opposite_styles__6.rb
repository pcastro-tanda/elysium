class TestOne
  protected def foo; end
end

class TestTwo
  protected
  ^^^^^^^^^ `protected` should be inlined in method definitions.
  def foo; end
end
