class Foo
  foo
  private attr_reader :bar, :baz
  ^^^^^^^ `private` should not be inlined in method definitions.
end
