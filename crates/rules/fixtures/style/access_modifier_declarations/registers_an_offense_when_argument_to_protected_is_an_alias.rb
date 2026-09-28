class Foo
  protected alias_method :bar, :foo
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
