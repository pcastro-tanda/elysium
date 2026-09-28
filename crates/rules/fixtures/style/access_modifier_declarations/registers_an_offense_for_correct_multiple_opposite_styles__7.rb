class TestOne
  public def foo; end
end

class TestTwo
  public
  ^^^^^^ `public` should be inlined in method definitions.
  def foo; end
end
