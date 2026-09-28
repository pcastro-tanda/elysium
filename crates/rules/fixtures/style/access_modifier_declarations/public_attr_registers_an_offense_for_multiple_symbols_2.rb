class Foo
  foo
  public attr :bar, :baz
  ^^^^^^ `public` should not be inlined in method definitions.
end
