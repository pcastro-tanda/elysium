class Foo
  foo
  protected attr :bar
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
