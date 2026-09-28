class TestOne
  private def foo; end
end

class TestTwo
  private
  ^^^^^^^ `private` should be inlined in method definitions.
  def foo; end
end
