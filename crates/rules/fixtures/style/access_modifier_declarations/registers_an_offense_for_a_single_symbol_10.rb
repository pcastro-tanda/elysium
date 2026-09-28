class Foo
  foo
  private attr_accessor :bar
  ^^^^^^^ `private` should not be inlined in method definitions.
end
