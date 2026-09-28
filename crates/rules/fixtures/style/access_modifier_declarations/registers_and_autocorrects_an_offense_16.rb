class Test
  def foo; end
  public :foo
  ^^^^^^ `public` should not be inlined in method definitions.
end
