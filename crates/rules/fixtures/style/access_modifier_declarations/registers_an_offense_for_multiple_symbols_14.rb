class Foo
  foo
  private attr_accessor :bar, :baz
  ^^^^^^^ `private` should not be inlined in method definitions.
end
