class Foo
  foo
  protected attr_writer :bar
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
