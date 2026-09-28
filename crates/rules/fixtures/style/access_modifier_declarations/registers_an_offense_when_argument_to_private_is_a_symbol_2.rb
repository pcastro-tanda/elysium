class Foo
  foo
  private :bar
  ^^^^^^^ `private` should not be inlined in method definitions.
end
