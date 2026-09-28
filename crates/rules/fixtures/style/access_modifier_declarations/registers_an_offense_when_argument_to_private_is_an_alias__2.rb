class Foo
  private alias_method :bar, :foo
  ^^^^^^^ `private` should not be inlined in method definitions.
end
