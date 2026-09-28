class Foo
  foo
  protected attr_accessor :bar
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
