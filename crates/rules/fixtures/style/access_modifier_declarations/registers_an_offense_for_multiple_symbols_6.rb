class Foo
  foo
  protected attr_accessor :bar, :baz
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
