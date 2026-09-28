class Foo
  foo
  private attr_writer :bar
  ^^^^^^^ `private` should not be inlined in method definitions.
end
