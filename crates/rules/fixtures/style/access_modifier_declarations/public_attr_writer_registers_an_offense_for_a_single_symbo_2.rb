class Foo
  foo
  public attr_writer :bar
  ^^^^^^ `public` should not be inlined in method definitions.
end
