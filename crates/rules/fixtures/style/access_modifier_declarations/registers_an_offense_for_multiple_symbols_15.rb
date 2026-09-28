class Foo
  foo
  protected attr_reader :bar, :baz
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
