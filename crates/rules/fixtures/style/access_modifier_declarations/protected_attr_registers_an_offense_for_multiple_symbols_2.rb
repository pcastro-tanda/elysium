class Foo
  foo
  protected attr :bar, :baz
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
