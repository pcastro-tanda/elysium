class Foo
  foo
  protected attr_writer :bar, :baz
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
