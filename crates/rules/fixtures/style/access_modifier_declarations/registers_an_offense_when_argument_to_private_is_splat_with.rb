class Foo
  foo
  private *%i[bar baz]
  ^^^^^^^ `private` should not be inlined in method definitions.
end
