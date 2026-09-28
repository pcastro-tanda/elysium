class Foo
  foo
  protected :bar, :baz
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
