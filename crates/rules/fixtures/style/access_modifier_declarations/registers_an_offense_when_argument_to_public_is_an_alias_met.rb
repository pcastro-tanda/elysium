class Foo
  public alias_method :bar, :foo
  ^^^^^^ `public` should not be inlined in method definitions.
end
