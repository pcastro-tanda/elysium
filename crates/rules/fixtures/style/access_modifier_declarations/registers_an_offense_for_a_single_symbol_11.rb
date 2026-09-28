class Foo
  foo
  protected attr_reader :bar
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
