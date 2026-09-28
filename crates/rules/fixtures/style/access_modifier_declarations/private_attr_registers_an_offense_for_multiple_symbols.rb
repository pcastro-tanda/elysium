class Foo
  foo
  private attr :bar, :baz
  ^^^^^^^ `private` should not be inlined in method definitions.
end
