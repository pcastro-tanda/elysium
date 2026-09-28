class TestOne
  public
end

class TestTwo
  public def foo; end
  ^^^^^^ `public` should not be inlined in method definitions.
end

class TestThree
  public def foo; end
  ^^^^^^ `public` should not be inlined in method definitions.
end
