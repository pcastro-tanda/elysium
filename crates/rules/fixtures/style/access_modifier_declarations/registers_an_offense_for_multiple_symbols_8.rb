class Foo
  foo
  module_function attr_reader :bar, :baz
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
end
