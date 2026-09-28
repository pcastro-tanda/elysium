class Foo
  foo
  module_function attr_accessor :bar, :baz
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
end
