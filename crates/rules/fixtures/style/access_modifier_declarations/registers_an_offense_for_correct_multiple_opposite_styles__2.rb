class TestOne
  protected
end

class TestTwo
  protected def foo; end
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end

class TestThree
  protected def foo; end
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
