class Foo
  foo
  public attr_accessor :bar, :baz
  ^^^^^^ `public` should not be inlined in method definitions.
end
