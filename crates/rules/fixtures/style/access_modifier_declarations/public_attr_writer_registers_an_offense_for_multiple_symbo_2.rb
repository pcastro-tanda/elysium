class Foo
  foo
  public attr_writer :bar, :baz
  ^^^^^^ `public` should not be inlined in method definitions.
end
