class TestOne
  private
end

class TestTwo
  private def foo; end
  ^^^^^^^ `private` should not be inlined in method definitions.
end

class TestThree
  private def foo; end
  ^^^^^^^ `private` should not be inlined in method definitions.
end
