class Foo
  foo
  private attr :bar
  ^^^^^^^ `private` should not be inlined in method definitions.
end
