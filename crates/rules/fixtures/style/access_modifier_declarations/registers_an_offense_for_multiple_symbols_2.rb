class Foo
  foo
  private attr_writer :bar, :baz
  ^^^^^^^ `private` should not be inlined in method definitions.
end
