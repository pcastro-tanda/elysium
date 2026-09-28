class Foo
  foo
  private :bar, :baz
  ^^^^^^^ `private` should not be inlined in method definitions.
end
