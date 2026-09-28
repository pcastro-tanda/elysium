class Foo
  foo
  public attr_reader :bar, :baz
  ^^^^^^ `public` should not be inlined in method definitions.
end
