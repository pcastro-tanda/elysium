class Foo
  foo
  protected *%i[bar baz]
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
