class Foo
  foo
  private attr_reader :bar
  ^^^^^^^ `private` should not be inlined in method definitions.
end
